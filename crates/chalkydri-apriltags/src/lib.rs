#![feature(
    portable_simd,
    alloc_layout_extra,
    slice_as_chunks,
    unchecked_math,
    sync_unsafe_cell,
    array_chunks
)]

use std::{
    fs::File,
    io::{Read, Write},
};

//mod detector;
//pub mod otsu;
//pub mod simd;
pub mod myalgo;

/*
use wgpu::{
    include_wgsl, Backends, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendState, ColorTargetState, ColorWrites,
    CommandEncoderDescriptor, Device, Extent3d, FragmentState, FrontFace, ImageCopyTexture,
    ImageDataLayout, Origin3d, PipelineLayoutDescriptor, PowerPreference, PrimitiveState,
    PrimitiveTopology, Queue, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor,
    RequestAdapterOptions, SamplerBindingType, ShaderModuleDescriptor, ShaderStages,
    StorageTextureAccess, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat,
    TextureSampleType, TextureUsages, TextureViewDimension, VertexState, BufferBindingType,
};

pub fn det(dev: Device, queue: Queue) {
    //dev.create_shader_module(ShaderModuleDescriptor { label: Some(", source: () })

    dev.start_capture();

    let texture_size = Extent3d {
        width: 300,
        height: 300,
        depth_or_array_layers: 1,
    };

    // we need to store this for later
let u32_size = std::mem::size_of::<u32>() as u32;

let output_buffer_size = (u32_size * 300 * 300) as wgpu::BufferAddress;
let output_buffer_desc = wgpu::BufferDescriptor {
    size: output_buffer_size,
    usage: wgpu::BufferUsages::COPY_DST
        // this tells wpgu that we want to read this buffer from the cpu
        | wgpu::BufferUsages::MAP_READ,
    label: None,
    mapped_at_creation: false,
};
let output_buffer = dev.create_buffer(&output_buffer_desc);


    let texture = dev.create_texture(&TextureDescriptor {
        size: Extent3d {
            width: 300,
            height: 300,
            depth_or_array_layers: 1,
        },
        format: TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        label: Some("texture"),
        view_formats: &[],
        dimension: TextureDimension::D2,
        mip_level_count: 1,
        sample_count: 1,
    });

    queue.write_texture(
        ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        image::open("test.png").unwrap().to_rgba8().to_vec().as_slice(),
        ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * 300),
            rows_per_image: Some(300),
        },
        texture_size,
    );

    let vert_shader = dev.create_shader_module(include_wgsl!("../shaders/vs.wgsl"));
    let sobel_shader = dev.create_shader_module(include_wgsl!("../shaders/sobel_shi_tomasi.wgsl"));

    let bind_group_layout = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("sobel_bind_group_layout"),
        entries: &[
            BindGroupLayoutEntry {
                ty: BindingType::Buffer { ty: BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                binding: 0,
                visibility: ShaderStages::VERTEX,
                count: None,
            },
        ],
    });

    let render_pipeline_layout = dev.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("Sobel Shi-Tomasi Pipline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
        //flags: PipelineLayoutFlags::TEST,
    });

    let render_pipeline = dev.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("Sobel Shi-Tomasi"),
        layout: Some(&render_pipeline_layout),
        vertex: VertexState {
            module: &vert_shader,
            entry_point: "vs_main",
            buffers: &[],
        },
        fragment: Some(FragmentState {
            module: &sobel_shader,
            entry_point: "fs_main",
            targets: &[Some(ColorTargetState {
                format: TextureFormat::Rgba8UnormSrgb,
                blend: Some(BlendState::REPLACE),
                write_mask: ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList, // 1.
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw, // 2.
            cull_mode: Some(wgpu::Face::Back),
            // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
            polygon_mode: wgpu::PolygonMode::Fill,
            // Requires Features::DEPTH_CLIP_CONTROL
            unclipped_depth: false,
            // Requires Features::CONSERVATIVE_RASTERIZATION
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
    });

    let mut enc = dev.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Cmd enc"),
    });
    {
        let mut pass = enc.begin_render_pass(&RenderPassDescriptor {
            label: Some("Sobel Shi-Tomasi Render Pass"),
            color_attachments: &[],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        pass.set_pipeline(&render_pipeline);
        //pass.draw_indexed(indices, base_vertex, instances)
    }

    enc.copy_texture_to_buffer(
    wgpu::ImageCopyTexture {
        aspect: wgpu::TextureAspect::All,
                texture: &texture,
        mip_level: 0,
        origin: wgpu::Origin3d::ZERO,
    },
    wgpu::ImageCopyBuffer {
        buffer: &output_buffer,
        layout: wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(u32_size * texture_size.height),
            rows_per_image: Some(texture_size.width),
        },
    },
    texture_size,
);


    queue.submit(&mut [enc.finish()].into_iter());

    image::save_buffer("out.png", &output_buffer.slice(..).get_mapped_range(), texture_size.width, texture_size.height, image::ColorType::Rgba8).unwrap();

    dev.stop_capture();

    //File::options().create(true).truncate(true).write(true).open("out.rgb").unwrap().write_all(texture.as_image_c);

    /*
    dev.create_pipeline_layout(wgpu::PipelineLayoutDescriptor { label: Some("apriltags-rs"), bind_group_layouts: &[], push_constant_ranges: () })
    let bind_group = dev.create_bind_group(&BindGroupDescriptor {
        label: Some("apriltags-rs"),
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::
        ],
    });
    */
}


*/
