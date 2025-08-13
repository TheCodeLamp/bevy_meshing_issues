use std::collections::BTreeSet;
use std::f32::consts::TAU;

use bevy::color::palettes::css::GREEN;
use bevy::color::palettes::css::WHITE;
use bevy::core_pipeline::core_3d::Transparent3d;
use bevy::ecs::query::QueryItem;
use bevy::ecs::system::SystemParamItem;
use bevy::ecs::system::lifetimeless::*;
use bevy::input::mouse::MouseMotion;
use bevy::pbr::MaterialPipeline;
use bevy::pbr::MeshPipeline;
use bevy::pbr::MeshPipelineKey;
use bevy::pbr::RenderMeshInstances;
use bevy::pbr::SetMaterialBindGroup;
use bevy::pbr::SetMeshBindGroup;
use bevy::pbr::SetMeshViewBindGroup;
use bevy::pbr::wireframe::WireframeConfig;
use bevy::pbr::wireframe::WireframePlugin;
use bevy::prelude::*;
use bevy::render::Render;
use bevy::render::RenderApp;
use bevy::render::RenderSet;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::extract_component::ExtractComponentPlugin;
use bevy::render::mesh::MeshVertexBufferLayoutRef;
use bevy::render::mesh::RenderMesh;
use bevy::render::mesh::RenderMeshBufferInfo;
use bevy::render::mesh::allocator::MeshAllocator;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_phase::AddRenderCommand;
use bevy::render::render_phase::DrawFunctions;
use bevy::render::render_phase::PhaseItem;
use bevy::render::render_phase::PhaseItemExtraIndex;
use bevy::render::render_phase::RenderCommand;
use bevy::render::render_phase::RenderCommandResult;
use bevy::render::render_phase::SetItemPipeline;
use bevy::render::render_phase::TrackedRenderPass;
use bevy::render::render_phase::ViewSortedRenderPhases;
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;
use bevy::render::sync_world::MainEntity;
use bevy::render::view::ExtractedView;
use bevy::render::view::NoFrustumCulling;
use binary_greedy_meshing::CS_P3;
use binary_greedy_meshing::Mesher;
use binary_greedy_meshing::pad_linearize;
use bytemuck::Pod;
use bytemuck::Zeroable;

// ---------- App ----------

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            CustomMaterialPlugin,
            WireframePlugin::default(),
        ))
        // // Wireframes can be configured with this resource. This can be changed at runtime.
        .insert_resource(WireframeConfig {
            // The global wireframe config enables drawing of wireframes on every mesh,
            // except those with `NoWireframe`. Meshes with `Wireframe` will always have a wireframe,
            // regardless of the global configuration.
            global: true,
            // Controls the default color of all wireframes. Used as the default color for global wireframes.
            // Can be changed per mesh using the `WireframeColor` component.
            default_color: WHITE.into(),
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (rotate, gizmos, move_camera, rotate_camera))
        .run();
}

fn quads() -> Vec<InstanceData> {
    let mut mesher = Mesher::new();
    let mut voxels = vec![0u16; CS_P3];
    let transparent_voxels = BTreeSet::new();
    voxels[pad_linearize(0, 0, 0)] = 1;
    let opaque_mask = binary_greedy_meshing::compute_opaque_mask(&voxels, &transparent_voxels);
    let transparent_mask =
        binary_greedy_meshing::compute_transparent_mask(&voxels, &transparent_voxels);
    mesher.fast_mesh(&voxels, &opaque_mask, &transparent_mask);

    // Generate encoded quads
    mesher
        .quads
        .into_iter()
        .enumerate()
        .flat_map(|(face, quads)| {
            let face = (face as u64) << 61;
            quads.into_iter().map(move |quad| face | quad)
        })
        // Flatten u64 -> [u32; 2] (lo, hi)
        .map(|quad| InstanceData {
            low: quad as u32,
            high: (quad >> 32) as u32,
        })
        .collect::<Vec<_>>()
}

// ---------- Systems ----------

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut transform = Transform::from_xyz(-2., 0., 0.).with_scale(Vec3::splat(0.5));
    transform.rotate_y(TAU * 0.5);
    transform.rotate_z(TAU * 0.5);
    meshes.add(Rectangle::new(1.0, 1.0));

    commands.spawn((
        Rotate,
        Transform::from_xyz(-2., 0., 0.).with_scale(Vec3::new(0.5, 0.5, 1.0)),
        Mesh3d(meshes.add(Rectangle::new(0.0, 0.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            metallic: 0.0,
            ..Default::default()
        })),
        InstanceMaterialData(quads()),
        // // NOTE: Frustum culling is done based on the Aabb of the Mesh and the GlobalTransform.
        // // As the cube is at the origin, if its Aabb moves outside the view frustum, all the
        // // instanced cubes will be culled.
        // // The InstanceMaterialData contains the 'GlobalTransform' information for this custom
        // // instancing, and that is not taken into account with the built-in frustum culling.
        // // We must disable the built-in frustum culling by adding the `NoFrustumCulling` marker
        // // component to avoid incorrect culling.
        NoFrustumCulling,
    ));
    commands.spawn((
        Rotate,
        Transform::from_xyz(2., 0., 0.).with_scale(Vec3::new(1.0, 0.5, 0.5)),
        Mesh3d(meshes.add(Rectangle::new(0.0, 0.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            metallic: 1.0,
            ..Default::default()
        })),
        InstanceMaterialData(quads()),
        // // NOTE: Frustum culling is done based on the Aabb of the Mesh and the GlobalTransform.
        // // As the cube is at the origin, if its Aabb moves outside the view frustum, all the
        // // instanced cubes will be culled.
        // // The InstanceMaterialData contains the 'GlobalTransform' information for this custom
        // // instancing, and that is not taken into account with the built-in frustum culling.
        // // We must disable the built-in frustum culling by adding the `NoFrustumCulling` marker
        // // component to avoid incorrect culling.
        NoFrustumCulling,
    ));

    // light
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));

    // camera
    commands.spawn((
        MainCamera,
        Camera3d::default(),
        Transform::from_xyz(4.0, 4.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn gizmos(mut gizmos: Gizmos) {
    gizmos.arrow(Vec3::new(2.0, 4.0, 0.0), Vec3::new(2.0, 1.0, 0.0), WHITE);
    gizmos.arrow(Vec3::new(-2.0, 4.0, 0.0), Vec3::new(-2.0, 1.0, 0.0), GREEN);
}

#[derive(Component)]
struct MainCamera;

const CAMERA_SPEED: f32 = 4.0;

fn move_camera(
    keys: Res<ButtonInput<KeyCode>>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
    time: Res<Time<Virtual>>,
) {
    let Ok(mut camera) = camera.get_single_mut() else {
        return;
    };

    let dir: Vec3 = {
        let mut out_dir = Vec3::default();

        if keys.pressed(KeyCode::KeyW) {
            out_dir += Vec3::new(0.0, 0.0, -1.0);
        }
        if keys.pressed(KeyCode::KeyA) {
            out_dir += Vec3::new(-1.0, 0.0, 0.0);
        }
        if keys.pressed(KeyCode::KeyS) {
            out_dir += Vec3::new(0.0, 0.0, 1.0);
        }
        if keys.pressed(KeyCode::KeyD) {
            out_dir += Vec3::new(1.0, 0.0, 0.0);
        }

        out_dir.normalize_or_zero()
    };

    let dir = camera.rotation.mul_vec3(dir).with_y(0.0);

    camera.translation += dir * time.delta_secs() * CAMERA_SPEED;
}

const CAMERA_ROT_SPEED: f32 = 0.001;

fn rotate_camera(
    mut mouse_motion: EventReader<MouseMotion>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
) {
    if !buttons.pressed(MouseButton::Left) {
        return;
    }

    let Ok(mut camera) = camera.get_single_mut() else {
        return;
    };

    mouse_motion.read().for_each(|mm| {
        let x = mm.delta.x;

        camera.rotate_axis(Dir3::Y, x * CAMERA_ROT_SPEED);
    });
}

#[derive(Component)]
struct Rotate;
fn rotate(time: Res<Time>, transforms: Query<&mut Transform, With<Rotate>>) {
    for transform in transforms {
        let refrence = transform.into_inner();
        // refrence.rotate_x(time.delta_secs() * TAU * 0.1);
        refrence.rotate_y(time.delta_secs() * TAU * 0.1);
        // refrence.rotate_z(time.delta_secs() * TAU * 0.5);
    }
}

#[derive(Component, Deref)]
// struct InstanceMaterialData(Vec<InstanceData>);
struct InstanceMaterialData(Vec<InstanceData>);

impl ExtractComponent for InstanceMaterialData {
    type QueryData = &'static InstanceMaterialData;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: QueryItem<'_, Self::QueryData>) -> Option<Self> {
        Some(InstanceMaterialData(item.0.clone()))
    }
}

struct CustomMaterialPlugin;

impl Plugin for CustomMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<InstanceMaterialData>::default());
        app.sub_app_mut(RenderApp)
            .add_render_command::<Transparent3d, DrawCustom>()
            .init_resource::<SpecializedMeshPipelines<CustomPipeline>>()
            .add_systems(
                Render,
                (
                    queue_custom.in_set(RenderSet::QueueMeshes),
                    prepare_instance_buffers.in_set(RenderSet::PrepareResources),
                ),
            );
    }

    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp).init_resource::<CustomPipeline>();
    }
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct InstanceData {
    low: u32,
    high: u32,
}

fn queue_custom(
    transparent_3d_draw_functions: Res<DrawFunctions<Transparent3d>>,
    custom_pipeline: Res<CustomPipeline>,
    mut pipelines: ResMut<SpecializedMeshPipelines<CustomPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<RenderMesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    material_meshes: Query<(Entity, &MainEntity), With<InstanceMaterialData>>,
    mut transparent_render_phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    views: Query<(&ExtractedView, &Msaa)>,
) {
    let draw_custom = transparent_3d_draw_functions.read().id::<DrawCustom>();

    for (view, msaa) in &views {
        let Some(transparent_phase) = transparent_render_phases.get_mut(&view.retained_view_entity)
        else {
            continue;
        };

        let msaa_key = MeshPipelineKey::from_msaa_samples(msaa.samples());

        let view_key = msaa_key | MeshPipelineKey::from_hdr(view.hdr);
        let rangefinder = view.rangefinder3d();
        for (entity, main_entity) in &material_meshes {
            let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(*main_entity)
            else {
                continue;
            };
            let Some(mesh) = meshes.get(mesh_instance.mesh_asset_id) else {
                continue;
            };
            let key =
                view_key | MeshPipelineKey::from_primitive_topology(mesh.primitive_topology());
            let pipeline = pipelines
                .specialize(&pipeline_cache, &custom_pipeline, key, &mesh.layout)
                .unwrap();
            // info!(%entity, "queing");
            transparent_phase.add(Transparent3d {
                entity: (entity, *main_entity),
                pipeline,
                draw_function: draw_custom,
                distance: rangefinder.distance_translation(&mesh_instance.translation),
                batch_range: 0..1,
                extra_index: PhaseItemExtraIndex::None,
                indexed: true,
            });
        }
    }
}

#[derive(Component)]
struct InstanceBuffer {
    buffer: Buffer,
    length: usize,
}

fn prepare_instance_buffers(
    mut commands: Commands,
    query: Query<(Entity, &InstanceMaterialData)>,
    render_device: Res<RenderDevice>,
) {
    // info!("preparing");
    for (entity, instance_data) in &query {
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("instance data buffer"),
            contents: bytemuck::cast_slice(instance_data.as_slice()),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        });
        commands.entity(entity).insert(InstanceBuffer {
            buffer,
            length: instance_data.len(),
        });
    }
}

#[derive(Resource)]
struct CustomPipeline {
    shader: Handle<Shader>,
    material_layout: BindGroupLayout,
    mesh_pipeline: MeshPipeline,
}

impl FromWorld for CustomPipeline {
    fn from_world(world: &mut World) -> Self {
        let mesh_pipeline = world.resource::<MeshPipeline>();
        let material_layout = world
            .resource::<MaterialPipeline<StandardMaterial>>()
            .material_layout
            .clone();

        CustomPipeline {
            shader: world.load_asset("shaders/voxel_rendering_instancing_poc.wgsl"),
            material_layout,
            mesh_pipeline: mesh_pipeline.clone(),
        }
    }
}

impl SpecializedMeshPipeline for CustomPipeline {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut descriptor = self.mesh_pipeline.specialize(key, layout)?;

        descriptor.vertex.shader_defs.push("BINDLESS".into());
        descriptor.vertex.shader_defs.push("VERTEX_COLORS".into());
        descriptor.vertex.shader = self.shader.clone();
        descriptor.vertex.buffers.push(VertexBufferLayout {
            array_stride: size_of::<InstanceData>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: vec![VertexAttribute {
                format: VertexFormat::Uint32x2,
                offset: 0,
                shader_location: 3, // shader locations 0-2 are taken up by Position, Normal and UV attributes
            }],
        });

        let fragment = descriptor.fragment.as_mut().unwrap();
        fragment.shader_defs.push("BINDLESS".into());
        fragment.shader_defs.push("VERTEX_COLORS".into());
        fragment.shader = self.shader.clone();

        assert_eq!(2, descriptor.layout.len());
        descriptor.layout.push(self.material_layout.clone());

        Ok(descriptor)
    }
}

type DrawCustom = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetMaterialBindGroup<StandardMaterial, 2>,
    DrawMeshInstanced,
);

struct DrawMeshInstanced;

impl<P: PhaseItem> RenderCommand<P> for DrawMeshInstanced {
    type Param = (
        SRes<RenderAssets<RenderMesh>>,
        SRes<RenderMeshInstances>,
        SRes<MeshAllocator>,
    );
    type ViewQuery = ();
    type ItemQuery = Read<InstanceBuffer>;

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        instance_buffer: Option<&'w InstanceBuffer>,
        (meshes, render_mesh_instances, mesh_allocator): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        // info!(entity = %item.entity(),"Draw command");
        // A borrow check workaround.
        let mesh_allocator = mesh_allocator.into_inner();

        let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(item.main_entity())
        else {
            return RenderCommandResult::Skip;
        };
        let Some(gpu_mesh) = meshes.into_inner().get(mesh_instance.mesh_asset_id) else {
            return RenderCommandResult::Skip;
        };
        let Some(instance_buffer) = instance_buffer else {
            return RenderCommandResult::Skip;
        };
        let Some(vertex_buffer_slice) =
            mesh_allocator.mesh_vertex_slice(&mesh_instance.mesh_asset_id)
        else {
            return RenderCommandResult::Skip;
        };
        // info!(mesh_id = ?mesh_instance.mesh_asset_id, entity = %item.entity(),"mesh id for entity");

        pass.set_vertex_buffer(0, vertex_buffer_slice.buffer.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.buffer.slice(..));

        match &gpu_mesh.buffer_info {
            RenderMeshBufferInfo::Indexed {
                index_format,
                count,
            } => {
                let Some(index_buffer_slice) =
                    mesh_allocator.mesh_index_slice(&mesh_instance.mesh_asset_id)
                else {
                    return RenderCommandResult::Skip;
                };

                pass.set_index_buffer(index_buffer_slice.buffer.slice(..), 0, *index_format);
                pass.draw_indexed(
                    index_buffer_slice.range.start..(index_buffer_slice.range.start + count),
                    vertex_buffer_slice.range.start as i32,
                    0..instance_buffer.length as u32,
                );
            }
            RenderMeshBufferInfo::NonIndexed => {
                pass.draw(vertex_buffer_slice.range, 0..instance_buffer.length as u32);
            }
        }
        RenderCommandResult::Success
    }
}
