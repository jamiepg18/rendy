//!
//! The mighty triangle example.
//! This examples shows colord triangle on white background.
//! Nothing fancy. Just prove that `rendy` works.
//!

#![cfg_attr(
    not(any(feature = "dx12", feature = "metal", feature = "vulkan")),
    allow(unused)
)]

use rendy::{
    command::{Families, QueueId, RenderPassEncoder},
    factory::{Config, Factory},
    graph::{
        present::PresentNode, render::*, Graph, GraphBuilder, GraphContext, NodeBuffer, NodeImage,
    },
    memory::Dynamic,
    mesh::PosColor,
    resource::{Buffer, BufferInfo, DescriptorSetLayout, Escape, Handle},
    shader::{ShaderKind, SourceLanguage, StaticShaderInfo},
};

#[cfg(feature = "spirv-reflection")]
use rendy::shader::SpirvReflection;

#[cfg(not(feature = "spirv-reflection"))]
use rendy::mesh::AsVertex;

use winit::{EventsLoop, WindowBuilder};

#[cfg(feature = "dx12")]
type Backend = rendy::dx12::Backend;

#[cfg(feature = "metal")]
type Backend = rendy::metal::Backend;

#[cfg(feature = "vulkan")]
type Backend = rendy::vulkan::Backend;

lazy_static::lazy_static! {
    static ref VERTEX: StaticShaderInfo = StaticShaderInfo::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/examples/triangle/shader.vert"),
        ShaderKind::Vertex,
        SourceLanguage::GLSL,
        "main",
    );

    static ref FRAGMENT: StaticShaderInfo = StaticShaderInfo::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/examples/triangle/shader.frag"),
        ShaderKind::Fragment,
        SourceLanguage::GLSL,
        "main",
    );

    static ref SHADERS: rendy::shader::ShaderSetBuilder = rendy::shader::ShaderSetBuilder::default()
        .with_vertex(&*VERTEX).unwrap()
        .with_fragment(&*FRAGMENT).unwrap();
}

#[cfg(feature = "spirv-reflection")]
lazy_static::lazy_static! {
    static ref SHADER_REFLECTION: SpirvReflection = SHADERS.reflect().unwrap();
}

#[derive(Debug, Default)]
struct TriangleRenderPipelineDesc;

#[derive(Debug)]
struct TriangleRenderPipeline<B: gfx_hal::Backend> {
    vertex: Option<Escape<Buffer<B>>>,
}

impl<B, T> SimpleGraphicsPipelineDesc<B, T> for TriangleRenderPipelineDesc
where
    B: gfx_hal::Backend,
    T: ?Sized,
{
    type Pipeline = TriangleRenderPipeline<B>;

    fn depth_stencil(&self) -> Option<gfx_hal::pso::DepthStencilDesc> {
        None
    }

    fn load_shader_set(&self, factory: &mut Factory<B>, _aux: &T) -> rendy_shader::ShaderSet<B> {
        SHADERS.build(factory).unwrap()
    }

    fn vertices(
        &self,
    ) -> Vec<(
        Vec<gfx_hal::pso::Element<gfx_hal::format::Format>>,
        gfx_hal::pso::ElemStride,
        gfx_hal::pso::VertexInputRate,
    )> {
        #[cfg(feature = "spirv-reflection")]
        return vec![SHADER_REFLECTION
            .attributes_range(..)
            .unwrap()
            .gfx_vertex_input_desc(gfx_hal::pso::VertexInputRate::Vertex)];

        #[cfg(not(feature = "spirv-reflection"))]
        return vec![PosColor::vertex().gfx_vertex_input_desc(gfx_hal::pso::VertexInputRate::Vertex)];
    }

    fn build<'a>(
        self,
        _ctx: &GraphContext<B>,
        _factory: &mut Factory<B>,
        _queue: QueueId,
        _aux: &T,
        buffers: Vec<NodeBuffer>,
        images: Vec<NodeImage>,
        set_layouts: &[Handle<DescriptorSetLayout<B>>],
    ) -> Result<TriangleRenderPipeline<B>, failure::Error> {
        assert!(buffers.is_empty());
        assert!(images.is_empty());
        assert!(set_layouts.is_empty());

        Ok(TriangleRenderPipeline { vertex: None })
    }
}

impl<B, T> SimpleGraphicsPipeline<B, T> for TriangleRenderPipeline<B>
where
    B: gfx_hal::Backend,
    T: ?Sized,
{
    type Desc = TriangleRenderPipelineDesc;

    fn prepare(
        &mut self,
        factory: &Factory<B>,
        _queue: QueueId,
        _set_layouts: &[Handle<DescriptorSetLayout<B>>],
        _index: usize,
        _aux: &T,
    ) -> PrepareResult {
        if self.vertex.is_none() {
            #[cfg(feature = "spirv-reflection")]
            let vbuf_size = SHADER_REFLECTION.attributes_range(..).unwrap().stride as u64 * 3;

            #[cfg(not(feature = "spirv-reflection"))]
            let vbuf_size = PosColor::vertex().stride as u64 * 3;

            let mut vbuf = factory
                .create_buffer(
                    BufferInfo {
                        size: vbuf_size,
                        usage: gfx_hal::buffer::Usage::VERTEX,
                    },
                    Dynamic,
                )
                .unwrap();

            unsafe {
                // Fresh buffer.
                factory
                    .upload_visible_buffer(
                        &mut vbuf,
                        0,
                        &[
                            PosColor {
                                position: [0.0, -0.5, 0.0].into(),
                                color: [1.0, 0.0, 0.0, 1.0].into(),
                            },
                            PosColor {
                                position: [0.5, 0.5, 0.0].into(),
                                color: [0.0, 1.0, 0.0, 1.0].into(),
                            },
                            PosColor {
                                position: [-0.5, 0.5, 0.0].into(),
                                color: [0.0, 0.0, 1.0, 1.0].into(),
                            },
                        ],
                    )
                    .unwrap();
            }

            self.vertex = Some(vbuf);
        }

        PrepareResult::DrawReuse
    }

    fn draw(
        &mut self,
        _layout: &B::PipelineLayout,
        mut encoder: RenderPassEncoder<'_, B>,
        _index: usize,
        _aux: &T,
    ) {
        let vbuf = self.vertex.as_ref().unwrap();
        encoder.bind_vertex_buffers(0, Some((vbuf.raw(), 0)));
        encoder.draw(0..3, 0..1);
    }

    fn dispose(self, _factory: &mut Factory<B>, _aux: &T) {}
}

#[cfg(any(feature = "dx12", feature = "metal", feature = "vulkan"))]
fn run(
    event_loop: &mut EventsLoop,
    factory: &mut Factory<Backend>,
    families: &mut Families<Backend>,
    mut graph: Graph<Backend, ()>,
) -> Result<(), failure::Error> {
    let started = std::time::Instant::now();

    let mut frames = 0u64..;
    let mut elapsed = started.elapsed();

    for _ in &mut frames {
        factory.maintain(families);
        event_loop.poll_events(|_| ());
        graph.run(factory, families, &mut ());

        elapsed = started.elapsed();
        if elapsed >= std::time::Duration::new(5, 0) {
            break;
        }
    }

    let elapsed_ns = elapsed.as_secs() * 1_000_000_000 + elapsed.subsec_nanos() as u64;

    log::info!(
        "Elapsed: {:?}. Frames: {}. FPS: {}",
        elapsed,
        frames.start,
        frames.start * 1_000_000_000 / elapsed_ns
    );

    graph.dispose(factory, &mut ());
    Ok(())
}

#[cfg(any(feature = "dx12", feature = "metal", feature = "vulkan"))]
fn main() {
    env_logger::Builder::from_default_env()
        .filter_module("triangle", log::LevelFilter::Trace)
        .init();

    let config: Config = Default::default();

    let (mut factory, mut families): (Factory<Backend>, _) = rendy::factory::init(config).unwrap();

    let mut event_loop = EventsLoop::new();

    let window = WindowBuilder::new()
        .with_title("Rendy example")
        .build(&event_loop)
        .unwrap();

    event_loop.poll_events(|_| ());

    let surface = factory.create_surface(window.into());

    let mut graph_builder = GraphBuilder::<Backend, ()>::new();

    let color = graph_builder.create_image(
        surface.kind(),
        1,
        factory.get_surface_format(&surface),
        Some(gfx_hal::command::ClearValue::Color(
            [1.0, 1.0, 1.0, 1.0].into(),
        )),
    );

    let pass = graph_builder.add_node(
        TriangleRenderPipeline::builder()
            .into_subpass()
            .with_color(color)
            .into_pass(),
    );

    graph_builder.add_node(PresentNode::builder(&factory, surface, color).with_dependency(pass));

    let graph = graph_builder
        .build(&mut factory, &mut families, &mut ())
        .unwrap();

    run(&mut event_loop, &mut factory, &mut families, graph).unwrap();
}

#[cfg(not(any(feature = "dx12", feature = "metal", feature = "vulkan")))]
fn main() {
    panic!("Specify feature: { dx12, metal, vulkan }");
}
