use std::sync::Mutex;

use kernel::wgpu;
use kernel::{Paint, Target};

const SHADER: &str = r#"
struct Inks { background: vec4<f32>, link: vec4<f32>, size: vec4<f32> };
@group(0) @binding(0) var<uniform> inks: Inks;

@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
  let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
  return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

@fragment fn fs(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
  let from_middle = (at.xy - inks.size.xy * 0.5) / inks.size.y;
  let r = length(from_middle);
  let ring = 0.5 + 0.5 * cos(r * 48.0);
  let fade = clamp(1.0 - r * 1.1, 0.0, 1.0);
  return mix(inks.background, inks.link, ring * fade);
}
"#;

/// Rings painted by a shader of Specimen's own, in the theme's background and link inks: the paint hook's example.
#[derive(Default)]
pub struct Rings {
    made: Mutex<Option<(u64, wgpu::RenderPipeline, wgpu::BindGroupLayout)>>,
}

impl Paint for Rings {
    fn stamp(&self) -> u64 {
        1
    }

    fn paint(&self, t: &Target) {
        let mut made = self.made.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if made.as_ref().is_none_or(|m| m.0 != t.gpu) {
            *made = Some(make(t));
        }
        let Some((_, pipeline, layout)) = made.as_ref() else { return };
        let inks: Vec<f32> = [t.theme.bg, t.theme.link, [t.width as f32, t.height as f32, 0.0, 0.0]].concat();
        let buffer = t.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (inks.len() * 4) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bytes: Vec<u8> = inks.iter().flat_map(|v| v.to_le_bytes()).collect();
        t.queue.write_buffer(&buffer, 0, &bytes);
        let group = t.device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }] });
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

fn make(t: &Target) -> (u64, wgpu::RenderPipeline, wgpu::BindGroupLayout) {
    let module = t.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
    let layout = t.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        }],
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
