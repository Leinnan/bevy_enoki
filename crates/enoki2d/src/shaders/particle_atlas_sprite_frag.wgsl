#import bevy_enoki::particle_vertex_out::{ VertexOutput }

@group(1) @binding(0) var texture: texture_2d<f32>;
@group(1) @binding(1) var texture_sampler: sampler;
@group(1) @binding(2) var<uniform> frame_data: vec4<f32>;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
	var out = in.color;

    let frame_width = frame_data.z;
    let frame_height = frame_data.w;
    let frame_offset = frame_data.xy;

    let uv = in.uv * vec2<f32>(frame_width, frame_height) + frame_offset;
	return out * textureSample(texture, texture_sampler, uv);
}
