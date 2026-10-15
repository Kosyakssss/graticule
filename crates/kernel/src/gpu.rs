use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};
use std::thread::JoinHandle;
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use winit::window::Window;

pub const ATLAS: u32 = 4096;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Quad {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub uv: [f32; 4],
    pub color: [f32; 4],
    pub clip: [f32; 4],
    pub kind: u32,
    pub image: u32,
    pub pad: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    viewport: [f32; 2],
    pad: [f32; 2],
}

const SHADER: &str = r#"
struct Globals { viewport: vec2<f32>, pad: vec2<f32> };
@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;
@group(1) @binding(0) var picture: texture_2d<f32>;
@group(1) @binding(1) var picture_sampler: sampler;

struct In {
  @builtin(vertex_index) vi: u32,
  @location(0) pos: vec2<f32>,
  @location(1) size: vec2<f32>,
  @location(2) uv: vec4<f32>,
  @location(3) color: vec4<f32>,
  @location(4) clip: vec4<f32>,
  @location(5) kind: u32,
};

struct Out {
  @builtin(position) position: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) color: vec4<f32>,
  @location(2) clip: vec4<f32>,
  @location(3) @interpolate(flat) kind: u32,
  @location(4) @interpolate(flat) span: vec2<f32>,
};

@vertex fn vs(input: In) -> Out {
  let corner = vec2<f32>(f32(input.vi & 1u), f32((input.vi >> 1u) & 1u));
  let p = input.pos + corner * input.size;
  var out: Out;
  out.position = vec4<f32>(p.x / globals.viewport.x * 2.0 - 1.0, 1.0 - p.y / globals.viewport.y * 2.0, 0.0, 1.0);
  out.uv = input.uv.xy + corner * input.uv.zw;
  out.color = input.color;
  out.clip = input.clip;
  out.kind = input.kind;
  out.span = input.uv.zw / max(input.size, vec2<f32>(1.0));
  return out;
}

@fragment fn fs(input: Out) -> @location(0) vec4<f32> {
  let p = input.position.xy;
  if (p.x < input.clip.x || p.y < input.clip.y || p.x >= input.clip.z || p.y >= input.clip.w) {
    discard;
  }
  if (input.kind == 2u) {
    let texels = vec2<f32>(textureDimensions(picture, 0)) * input.span;
    let lod = max(0.0, log2(max(texels.x, texels.y)));
    let c = textureSampleLevel(picture, picture_sampler, input.uv, lod);
    let pa = c.a * input.color.a;
    return vec4<f32>(c.rgb * pa, pa);
  }
  var a = input.color.a;
  if (input.kind == 1u) {
    a = a * textureSampleLevel(atlas, atlas_sampler, input.uv, 0.0).r;
  }
  return vec4<f32>(input.color.rgb * a, a);
}
"#;

pub struct Gpu {
    id: u64,
    surface: Option<wgpu::Surface<'static>>,
    format: wgpu::TextureFormat,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    globals: wgpu::Buffer,
    atlas: wgpu::Texture,
    instances: wgpu::Buffer,
    capacity: usize,
    picture_layout: wgpu::BindGroupLayout,
    picture_sampler: wgpu::Sampler,
    blank: wgpu::BindGroup,
    pictures: std::collections::HashMap<u32, (wgpu::Texture, wgpu::BindGroup)>,
    runs: Vec<(u32, std::ops::Range<u32>)>,
    kept: [(wgpu::Buffer, usize, usize); 4],
    kept_runs: [Vec<(u32, std::ops::Range<u32>)>; 4],
    pending: Vec<crate::canvas::Raster>,
    arriving: Vec<crate::image::Image>,
    hooks: Vec<crate::canvas::Hooked>,
    watched: Vec<(u32, std::sync::Weak<crate::image::Pixels>)>,
    next: Next,
    clear: [f32; 4],
}

#[derive(Clone, Copy)]
enum Next {
    Kept,
    Full(usize),
}

impl Gpu {
    pub fn new(window: Option<Arc<Window>>, size: (u32, u32)) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = window.map(|w| instance.create_surface(w).expect("surface"));
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: surface.as_ref(),
            apply_limit_buckets: false,
        }))
        .expect("adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .expect("device");

        let (format, alpha_mode) = match &surface {
            Some(surface) => {
                let caps = surface.get_capabilities(&adapter);
                let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
                (format, caps.alpha_modes[0])
            }
            None => (wgpu::TextureFormat::Rgba8Unorm, wgpu::CompositeAlphaMode::Opaque),
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.0.max(1),
            height: size.1.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 1,
            alpha_mode,
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        if let Some(surface) = &surface {
            surface.configure(&device, &config);
        }

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(SHADER.into()) });

        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: ATLAS, height: ATLAS, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Nearest, min_filter: wgpu::FilterMode::Nearest, ..Default::default() });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&atlas_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        let picture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let picture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let blank = Self::picture_group(&device, &queue, &picture_layout, &picture_sampler, &[(1, 1, &[0, 0, 0, 0])]).1;
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&layout), Some(&picture_layout)], immediate_size: 0 });
        let attributes = wgpu::vertex_attr_array![
            0 => Float32x2,
            1 => Float32x2,
            2 => Float32x4,
            3 => Float32x4,
            4 => Float32x4,
            5 => Uint32
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout { array_stride: size_of::<Quad>() as u64, step_mode: wgpu::VertexStepMode::Instance, attributes: &attributes })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let capacity = 4096;
        let instances = Self::instance_buffer(&device, capacity);
        let kept = std::array::from_fn(|_| (Self::instance_buffer(&device, capacity), capacity, 0));

        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self {
            id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            surface,
            format,
            device,
            queue,
            config,
            pipeline,
            bind_group,
            globals,
            atlas,
            instances,
            capacity,
            picture_layout,
            picture_sampler,
            blank,
            pictures: std::collections::HashMap::new(),
            kept_runs: Default::default(),
            pending: Vec::new(),
            arriving: Vec::new(),
            hooks: Vec::new(),
            watched: Vec::new(),
            runs: Vec::new(),
            kept,
            next: Next::Full(0),
            clear: [0.0; 4],
        }
    }

    fn picture_group(device: &wgpu::Device, queue: &wgpu::Queue, layout: &wgpu::BindGroupLayout, sampler: &wgpu::Sampler, levels: &[(u32, u32, &[u8])]) -> (wgpu::Texture, wgpu::BindGroup) {
        let (width, height, _) = levels[0];
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count: levels.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (level, &(w, h, rgba)) in levels.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: level as u32, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                rgba,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
                wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            );
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
            ],
        });
        (texture, group)
    }

    #[profiling::function]
    pub fn add_image(&mut self, id: u32, width: u32, height: u32, rgba: &[u8]) {
        let entry = Self::picture_group(&self.device, &self.queue, &self.picture_layout, &self.picture_sampler, &[(width, height, rgba)]);
        self.pictures.insert(id, entry);
    }

    /// Puts `rgba` in the texture `id`, writing over it when it is already this size.
    pub fn set_image(&mut self, id: u32, width: u32, height: u32, rgba: &[u8]) {
        match self.pictures.get(&id) {
            Some((t, _)) if t.width() == width && t.height() == height => self.queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: t, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                rgba,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(width * 4), rows_per_image: Some(height) },
                wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            ),
            _ => {
                self.remove_image(id);
                self.add_image(id, width, height, rgba);
            }
        }
    }

    /// Takes a canvas to draw before the next frame is shown.
    pub fn raster(&mut self, r: crate::canvas::Raster) {
        self.pending.retain(|p| p.id != r.id);
        self.pending.push(r);
    }

    /// Takes an image to send to the GPU before the next frame is shown. Its texture lasts as long as the image does.
    pub fn picture(&mut self, image: crate::image::Image) {
        self.arriving.push(image);
    }

    /// Takes a canvas's paint hook to run before the next frame is shown.
    pub fn hook(&mut self, h: crate::canvas::Hooked) {
        self.hooks.retain(|p| p.id != h.id);
        self.hooks.push(h);
    }

    fn target(&mut self, id: u32, width: u32, height: u32) -> wgpu::TextureView {
        let fits = self.pictures.get(&id).is_some_and(|(t, _)| t.width() == width && t.height() == height && t.usage().contains(wgpu::TextureUsages::RENDER_ATTACHMENT));
        if !fits {
            self.remove_image(id);
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.picture_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.picture_sampler) },
                ],
            });
            self.pictures.insert(id, (texture, group));
        }
        self.pictures[&id].0.create_view(&wgpu::TextureViewDescriptor::default())
    }

    #[profiling::function]
    fn settle(&mut self) {
        for h in std::mem::take(&mut self.hooks) {
            if h.width == 0 || h.height == 0 {
                continue;
            }
            let view = self.target(h.id, h.width, h.height);
            h.paint.paint(&crate::canvas::Target {
                gpu: self.id,
                device: &self.device,
                queue: &self.queue,
                view: &view,
                format: wgpu::TextureFormat::Rgba8Unorm,
                width: h.width,
                height: h.height,
                theme: h.theme,
            });
        }
        for r in std::mem::take(&mut self.pending) {
            if r.scale.width > 0 && r.scale.height > 0 {
                let rgba = r.rgba();
                self.set_image(r.id, r.scale.width, r.scale.height, &rgba);
            }
        }
        let most = self.device.limits().max_texture_dimension_2d;
        for image in std::mem::take(&mut self.arriving) {
            let Some(pixels) = image.pixels() else { continue };
            let levels = crate::image::levels(pixels, most);
            let levels: Vec<(u32, u32, &[u8])> = levels.iter().map(|(w, h, d)| (*w, *h, d.as_slice())).collect();
            let entry = Self::picture_group(&self.device, &self.queue, &self.picture_layout, &self.picture_sampler, &levels);
            self.remove_image(image.texture());
            self.pictures.insert(image.texture(), entry);
            self.watched.push((image.texture(), image.watch()));
        }
        let gone: Vec<u32> = self.watched.iter().filter(|(_, w)| w.strong_count() == 0).map(|(id, _)| *id).collect();
        if !gone.is_empty() {
            self.watched.retain(|(_, w)| w.strong_count() > 0);
            for id in gone {
                self.remove_image(id);
            }
        }
    }

    pub fn remove_image(&mut self, id: u32) {
        if let Some((t, _)) = self.pictures.remove(&id) {
            t.destroy();
        }
    }

    fn instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (capacity * size_of::<Quad>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.config);
        }
    }

    pub fn upload(&self, x: u32, y: u32, w: u32, h: u32, data: &[u8]) {
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &self.atlas, mip_level: 0, origin: wgpu::Origin3d { x, y, z: 0 }, aspect: wgpu::TextureAspect::All },
            data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
    }

    fn prepare(&mut self, quads: &[Quad], width: u32, height: u32) {
        if quads.len() > self.capacity {
            self.capacity = quads.len().next_power_of_two();
            self.instances = Self::instance_buffer(&self.device, self.capacity);
        }
        self.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&Globals { viewport: [width as f32, height as f32], pad: [0.0; 2] }));
        self.queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(quads));
        runs_of(quads, &mut self.runs);
    }

    fn encode(&self, view: &wgpu::TextureView, count: usize, clear: [f32; 4]) -> wgpu::CommandEncoder {
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: clear[0] as f64, g: clear[1] as f64, b: clear[2] as f64, a: clear[3] as f64 }), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.instances.slice(..));
            for (image, range) in &self.runs {
                let group = self.pictures.get(image).map(|(_, g)| g).unwrap_or(&self.blank);
                pass.set_bind_group(1, group, &[]);
                let end = range.end.min(count as u32);
                if range.start < end {
                    pass.draw(0..4, range.start..end);
                }
            }
        }
        encoder
    }

    /// Sends the kept layers' changed quads to the GPU: only the dirty range of each, unless a layer outgrew its buffer.
    #[profiling::function]
    pub fn keep(&mut self, layers: [&[Quad]; 4], dirty: [Option<std::ops::Range<usize>>; 4]) -> usize {
        let mut sent = 0;
        for (k, quads) in layers.into_iter().enumerate() {
            let (buffer, cap, len) = &mut self.kept[k];
            if quads.len() > *cap || dirty[k].as_ref().is_some_and(|d| !d.is_empty()) || quads.len() != *len {
                runs_of(quads, &mut self.kept_runs[k]);
            }
            if quads.len() > *cap {
                *cap = quads.len().next_power_of_two();
                *buffer = Self::instance_buffer(&self.device, *cap);
                self.queue.write_buffer(buffer, 0, bytemuck::cast_slice(quads));
                sent += quads.len();
            } else if let Some(d) = &dirty[k] {
                let d = d.start.min(quads.len())..d.end.min(quads.len());
                if !d.is_empty() {
                    self.queue.write_buffer(buffer, (d.start * size_of::<Quad>()) as u64, bytemuck::cast_slice(&quads[d.clone()]));
                    sent += d.len();
                }
            }
            *len = quads.len();
        }
        sent
    }

    /// Makes the kept layers the next frame.
    pub fn show_kept(&mut self, clear: [f32; 4]) {
        self.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&Globals { viewport: [self.config.width as f32, self.config.height as f32], pad: [0.0; 2] }));
        self.next = Next::Kept;
        self.clear = clear;
    }

    /// Makes `quads` the next frame.
    pub fn show(&mut self, quads: &[Quad], clear: [f32; 4]) {
        self.prepare(quads, self.config.width, self.config.height);
        self.next = Next::Full(quads.len());
        self.clear = clear;
    }

    fn draw(&mut self, view: &wgpu::TextureView) {
        self.settle();
        let encoder = match self.next {
            Next::Kept => self.encode_kept(view, self.clear),
            Next::Full(count) => self.encode(view, count, self.clear),
        };
        self.queue.submit([encoder.finish()]);
    }

    fn encode_kept(&self, view: &wgpu::TextureView, clear: [f32; 4]) -> wgpu::CommandEncoder {
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: clear[0] as f64, g: clear[1] as f64, b: clear[2] as f64, a: clear[3] as f64 }), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            for ((buffer, _, len), runs) in self.kept.iter().zip(&self.kept_runs) {
                if *len == 0 {
                    continue;
                }
                pass.set_vertex_buffer(0, buffer.slice(..));
                for (image, range) in runs {
                    pass.set_bind_group(1, self.pictures.get(image).map(|(_, g)| g).unwrap_or(&self.blank), &[]);
                    let end = range.end.min(*len as u32);
                    if range.start < end {
                        pass.draw(0..4, range.start..end);
                    }
                }
            }
        }
        encoder
    }

    pub fn snapshot(&mut self, quads: &[Quad], clear: [f32; 4]) -> Vec<u8> {
        self.settle();
        let (width, height) = (self.config.width, self.config.height);
        self.prepare(quads, width, height);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let row = (width * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.encode(&view, quads.len(), clear);
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(height) } },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);
        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        let mapped = buffer.slice(..).get_mapped_range().expect("mapped");
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            let start = (y * row) as usize;
            pixels.extend_from_slice(&mapped[start..start + (width * 4) as usize]);
        }
        pixels
    }
}

fn runs_of(quads: &[Quad], runs: &mut Vec<(u32, std::ops::Range<u32>)>) {
    runs.clear();
    let mut bound = 0u32;
    let mut start = 0u32;
    for (i, q) in quads.iter().enumerate() {
        if q.kind == 2 && q.image != bound {
            if i as u32 > start {
                runs.push((bound, start..i as u32));
            }
            bound = q.image;
            start = i as u32;
        }
    }
    if (quads.len() as u32) > start {
        runs.push((bound, start..quads.len() as u32));
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A window's GPU. The main thread stages each frame and returns; a presenter thread of its own waits for the display to take a frame, then draws whatever was staged last, so the main thread never waits for the display.
pub struct Screen {
    gpu: Arc<Mutex<Gpu>>,
    surface: Arc<Mutex<wgpu::Surface<'static>>>,
    waiting: Arc<AtomicBool>,
    shown: Arc<Mutex<Option<Instant>>>,
    ready: Option<mpsc::SyncSender<()>>,
    presenter: Option<JoinHandle<()>>,
}

impl Screen {
    /// Opens the GPU for `window`. `presented` is called on the presenter thread each time it has handed a frame to the display.
    pub fn new(window: Arc<Window>, size: (u32, u32), presented: impl Fn() + Send + 'static) -> Self {
        let mut gpu = Gpu::new(Some(window), size);
        let surface = Arc::new(Mutex::new(gpu.surface.take().expect("a window has a surface")));
        let gpu = Arc::new(Mutex::new(gpu));
        let waiting = Arc::new(AtomicBool::new(false));
        let shown = Arc::new(Mutex::new(None));
        let (ready, staged) = mpsc::sync_channel::<()>(1);
        let presenter = {
            let (gpu, surface, waiting, shown) = (gpu.clone(), surface.clone(), waiting.clone(), shown.clone());
            std::thread::Builder::new()
                .name("graticule presenter".into())
                .spawn(move || {
                    while staged.recv().is_ok() {
                        present(&gpu, &lock(&surface), &waiting, &shown);
                        presented();
                    }
                })
                .ok()
        };
        Self { gpu, surface, waiting, shown, ready: Some(ready), presenter }
    }

    /// When the display last took a frame: about when it last refreshed.
    pub fn shown(&self) -> Option<Instant> {
        *lock(&self.shown)
    }

    /// Whether the last staged frame is still waiting for the display.
    pub fn waiting(&self) -> bool {
        self.waiting.load(Ordering::Acquire)
    }

    /// Stages the next frame and hands it to the presenter. The presenter draws whatever is staged when the display is ready, so frames staged in between are never drawn.
    pub fn stage(&self, f: impl FnOnce(&mut Gpu)) {
        f(&mut lock(&self.gpu));
        self.waiting.store(true, Ordering::Release);
        if let Some(ready) = &self.ready {
            let _ = ready.try_send(());
        }
    }

    /// Presents the staged frame on this thread, waiting for the display: for the first frame and a resize, which must be on screen before the event returns.
    pub fn present_now(&self) {
        present(&self.gpu, &lock(&self.surface), &self.waiting, &self.shown);
    }

    pub fn resize(&self, width: u32, height: u32) {
        let surface = lock(&self.surface);
        let mut gpu = lock(&self.gpu);
        gpu.config.width = width.max(1);
        gpu.config.height = height.max(1);
        surface.configure(&gpu.device, &gpu.config);
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        self.ready = None;
        if let Some(presenter) = self.presenter.take() {
            let _ = presenter.join();
        }
    }
}

fn present(gpu: &Mutex<Gpu>, surface: &wgpu::Surface<'static>, waiting: &AtomicBool, shown: &Mutex<Option<Instant>>) {
    let frame = match surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(frame) | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
            *lock(shown) = Some(Instant::now());
            frame
        }
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            let gpu = lock(gpu);
            surface.configure(&gpu.device, &gpu.config);
            waiting.store(false, Ordering::Release);
            return;
        }
        _ => {
            waiting.store(false, Ordering::Release);
            return;
        }
    };
    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
    let queue = {
        let mut gpu = lock(gpu);
        waiting.store(false, Ordering::Release);
        gpu.draw(&view);
        gpu.queue.clone()
    };
    queue.present(frame);
}
