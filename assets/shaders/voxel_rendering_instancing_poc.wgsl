#import bevy_pbr::mesh_functions::{get_world_from_local, mesh_position_local_to_clip},
#import bevy_pbr::mesh_view_bindings::globals
#import bevy_pbr::forward_io::{VertexOutput, FragmentOutput},
#import bevy_pbr::pbr_fragment::pbr_input_from_standard_material,
#import bevy_pbr::pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing, alpha_discard},
#import bevy_pbr::view_transformations::position_world_to_clip,
#import bevy_pbr::pbr_bindings::material;


struct Vertex {
    @builtin(vertex_index)   vertex_index: u32,
    @builtin(instance_index) instance_index: u32,

    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,

    @location(3) quad: vec2<u32>,
};

struct Decoded {
    x: f32, y: f32, z: f32,
    w: f32, h: f32,
    voxel: u32,
    face_direction: u32, // 0..5 => Up, Down, Right, Left, Front, Back
};

fn decode_pair(p: vec2<u32>) -> Decoded {
    let lo = p.x;
    let hi = p.y;

    let x = f32((lo >>  0u) & 0x3Fu);
    let y = f32((lo >>  6u) & 0x3Fu);
    let z = f32((lo >> 12u) & 0x3Fu);
    let w = f32((lo >> 18u) & 0x3Fu);
    let h = f32((lo >> 24u) & 0x3Fu);

    let voxel =  hi & 0xFFFFu;
    let face_direction  = (hi >> 29u) & 0x7u;

    return Decoded(x, y, z, w, h, voxel, face_direction);
}

struct Frame {
  p0: vec3<f32>,
  u:  vec3<f32>,
  v:  vec3<f32>,
}

fn face_frame(d: Decoded) -> Frame {
  let xf = f32(d.x);
  let yf = f32(d.y);
  let zf = f32(d.z);
  let wf = f32(d.w);
  let hf = f32(d.h);

  switch (d.face_direction) {
    // Up (Y+)
    case 0u { return Frame(vec3(xf, yf, zf), vec3(wf, 0.0, 0.0), vec3(0.0, 0.0,  hf)); }
    // Down (Y-)
    case 1u { return Frame(vec3(xf, yf, zf), vec3(-wf, 0.0, 0.0), vec3(0.0, 0.0,  hf)); }
    // Right (X+)
    case 2u { return Frame(vec3(xf, yf, zf), vec3(0.0, -wf, 0.0), vec3(0.0, 0.0,  hf)); }
    // Left (X-)
    case 3u { return Frame(vec3(xf, yf, zf), vec3(0.0,  wf, 0.0), vec3(0.0, 0.0,  hf)); }
    // Front (Z+)
    case 4u { return Frame(vec3(xf, yf, zf), vec3(-wf, 0.0, 0.0), vec3(0.0,  hf, 0.0)); }
    // Back (Z-)
    default { return Frame(vec3(xf, yf, zf), vec3( wf, 0.0, 0.0), vec3(0.0,  hf, 0.0)); }
  }
}

fn corner_uv(vert_in_quad: u32) -> vec2<f32> {
  // Triangles: [0,1,2] and [0,2,3] — matches your Rust order
  switch (vert_in_quad) {
    case 0u { return vec2(0.0, 0.0); }
    case 1u { return vec2(0.0, 1.0); }
    case 2u { return vec2(1.0, 1.0); }
    case 3u { return vec2(0.0, 0.0); }
    case 4u { return vec2(1.0, 1.0); }
    case default { return vec2(1.0, 0.0); }
  };
}


@vertex
fn vertex(v: Vertex) -> VertexOutput {
    var out: VertexOutput;

    let vert_in_quad = v.vertex_index % 6u;
    let decoded_quad = decode_pair(v.quad);
    let frame = face_frame(decoded_quad);               // gives p0, u, v in LOCAL space
    let uv = corner_uv(vert_in_quad);

    let local_pos = frame.p0 + frame.u * uv.x + frame.v * uv.y;
    let local_n   = normalize(cross(frame.v, frame.u));

    let world_from_local = get_world_from_local(0u);

    let world_pos = (world_from_local * vec4(local_pos, 1.0)).xyz + v.position.xyz;
    let world_n   = normalize((world_from_local * vec4(local_n, 0.0)).xyz);

    // required fields
    out.position        = position_world_to_clip(world_pos);
    out.world_position  = vec4(world_pos, 1.0);
    out.world_normal    = world_n;

    // optional fields (only if the struct actually has them)
    #ifdef VERTEX_UVS_A
    out.uv = uv;
    #endif

    #ifdef VERTEX_TANGENTS
    // if you don't use normal maps, a dummy tangent is fine
    out.world_tangent = vec4(0.0, 0.0, 0.0, 1.0);
    #endif

    #ifdef VERTEX_COLORS
    let a = 1.0;
    switch (decoded_quad.face_direction) {
        case 0u { out.color = vec4(0.0, 1.0, 0.0, a); }
        case 1u { out.color = vec4(1.0, 0.0, 1.0, a); }
        case 2u { out.color = vec4(1.0, 0.0, 0.0, a); }
        case 3u { out.color = vec4(0.0, 1.0, 1.0, a); }
        case 4u { out.color = vec4(0.0, 0.0, 1.0, a); }
        case 5u { out.color = vec4(1.0, 1.0, 0.0, a); }
        case default { out.color = vec4(1.0, 1.0, 1.0, a); }
    }
    // Color for all face directions
    // out.color = vec4(0.05, 0.05, 0.4, a);
    #endif

    return out;
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var out: FragmentOutput;
    out.color = in.color;

    // Build PBR inputs from StandardMaterial + varyings
    var pbr_in = pbr_input_from_standard_material(in, is_front);

    // Alpha discard, lighting, post-processing (fog/tonemap), etc.
    pbr_in.material.base_color = alpha_discard(pbr_in.material, pbr_in.material.base_color);
    out.color = apply_pbr_lighting(pbr_in);
    out.color = main_pass_post_lighting_processing(pbr_in, out.color);
    
    return out;
}
