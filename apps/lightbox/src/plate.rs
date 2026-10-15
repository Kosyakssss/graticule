use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, PoisonError};

use graticule::kernel::{Camera, Paint, Target, wgpu};
use graticule::prelude::*;

use crate::scan::Scan;

const SHADER: &str = r#"
struct View { camera: vec4<f32>, levels: vec4<f32>, size: vec4<f32>, ramp: array<vec4<f32>, 5> };
@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var scan: texture_2d<u32>;

@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
  let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
  return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

fn at(p: vec2<f32>) -> vec4<f32> {
  if (p.x < 0.0 || p.y < 0.0 || p.x >= view.size.x || p.y >= view.size.y) {
    return vec4<f32>(0.0);
  }
  let v = textureLoad(scan, vec2<i32>(floor(p)), 0);
  return vec4<f32>(vec3<f32>(v.rgb) / 65535.0, 1.0);
}

@fragment fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
  let step = 1.0 / (view.camera.z * view.camera.w);
  let p = view.camera.xy + pos.xy * step;
  var sum = at(p);
  if (step > 1.0) {
    let q = step * 0.25;
    sum = (at(p + vec2<f32>(-q, -q)) + at(p + vec2<f32>(q, -q)) + at(p + vec2<f32>(-q, q)) + at(p + vec2<f32>(q, q))) * 0.25;
  }
  if (sum.a <= 0.0) {
    return vec4<f32>(0.0);
  }
  var v = clamp((sum.rgb / sum.a - view.levels.x) / max(view.levels.y - view.levels.x, 0.000001), vec3<f32>(0.0), vec3<f32>(1.0));
  v = pow(max(v, vec3<f32>(0.000001)), vec3<f32>(1.0 / view.levels.z));
  if (view.levels.w > 1.5) {
    let g = (v.r + v.g + v.b) / 3.0 * 4.0;
    let i = min(u32(floor(g)), 3u);
    return vec4<f32>(mix(view.ramp[i].rgb, view.ramp[i + 1u].rgb, g - f32(i)), sum.a);
  }
  if (view.levels.w > 0.5) {
    v = vec3<f32>(1.0) - v;
  }
  return vec4<f32>(v, sum.a);
}
"#;

/// How a scan's values become colors.
#[derive(Facts, Choice, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Shown {
    #[default]
    Positive,
    Negative,
    /// From the background through the link, ok, warn and danger inks, so a false-color scan follows the theme.
    FalseColor,
}

/// A scan painted on the GPU at its full depth, through the canvas's camera, with its levels applied: black and white points from 0 to 65535, gamma in hundredths, and how its values become colors.
pub struct Plate {
    pub scan: Arc<Scan>,
    pub camera: Camera,
    pub levels: (u16, u16),
    pub gamma: u16,
    pub shown: Shown,
    /// The canvas's width in points, so the camera's points become the texture's pixels.
    pub width: f64,
}

static MADE: Mutex<Option<(u64, wgpu::RenderPipeline, wgpu::BindGroupLayout)>> = Mutex::new(None);

impl Paint for Plate {
    fn stamp(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (self.scan.id, self.levels, self.gamma, self.shown).hash(&mut h);
        for v in [self.camera.x, self.camera.y, self.camera.zoom, self.width] {
            v.to_bits().hash(&mut h);
        }
        h.finish()
    }

    fn paint(&self, t: &Target) {
        let mut made = MADE.lock().unwrap_or_else(PoisonError::into_inner);
        if made.as_ref().is_none_or(|m| m.0 != t.gpu) {
            *made = Some(make(t));
        }
        let Some((_, pipeline, layout)) = made.as_ref() else { return };
        let mut held = self.scan.gpu.lock().unwrap_or_else(PoisonError::into_inner);
        if held.as_ref().is_none_or(|h| h.0 != t.gpu) {
            *held = Some((t.gpu, upload(t, &self.scan)));
        }
        let Some((_, texture)) = held.as_ref() else { return };
        let ratio = if self.width > 0.0 { f64::from(t.width) / self.width } else { 1.0 };
        let (black, white) = (f32::from(self.levels.0) / 65_535.0, f32::from(self.levels.1) / 65_535.0);
        let ramp = [t.theme.bg, t.theme.link, t.theme.ok, t.theme.warn, t.theme.danger];
        let numbers: Vec<f32> = [
            self.camera.x as f32,
            self.camera.y as f32,
            self.camera.zoom as f32,
            ratio as f32,
            black,
            white,
            f32::from(self.gamma.max(1)) / 100.0,
            match self.shown {
                Shown::Positive => 0.0,
                Shown::Negative => 1.0,
                Shown::FalseColor => 2.0,
            },
            self.scan.width as f32,
            self.scan.height as f32,
            0.0,
            0.0,
        ]
        .into_iter()
        .chain(ramp.into_iter().flatten())
        .collect();
        let buffer = t.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (numbers.len() * 4) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bytes: Vec<u8> = numbers.iter().flat_map(|v| v.to_le_bytes()).collect();
        t.queue.write_buffer(&buffer, 0, &bytes);
        let seen = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let group = t.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&seen) }],
        });
        let mut encoder = t.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: t.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }
        t.queue.submit([encoder.finish()]);
    }
}

fn upload(t: &Target, scan: &Scan) -> wgpu::Texture {
    let size = wgpu::Extent3d { width: scan.width, height: scan.height, depth_or_array_layers: 1 };
    let texture = t.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Uint,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    t.queue.write_texture(
        wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        &scan.rgba(),
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(scan.width * 8), rows_per_image: Some(scan.height) },
        size,
    );
    texture
}

fn make(t: &Target) -> (u64, wgpu::RenderPipeline, wgpu::BindGroupLayout) {
    let module = t.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
    let layout = t.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Uint, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                count: None,
            },
        ],
    });
    let pipeline_layout = t.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
    let pipeline = t.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { format: t.format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });
    (t.gpu, pipeline, layout)
}
