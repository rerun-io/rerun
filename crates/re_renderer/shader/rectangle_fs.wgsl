#import <./colormap.wgsl>
#import <./rectangle.wgsl>
#import <./utils/srgb.wgsl>
#import <./decodings.wgsl>

// WARNING! Adding anything else to this shader is very likely to push us over a size threshold that
// causes the failure reported in: https://github.com/rerun-io/rerun/issues/3931
// Make sure any changes are tested in Chrome on Linux using the Intel Mesa driver.

fn is_magnifying(pixel_coord: vec2f) -> bool {
    return fwidth(pixel_coord.x) < 1.0;
}

fn tex_filter(pixel_coord: vec2f) -> u32 {
    if is_magnifying(pixel_coord) {
        return rect_info.magnification_filter;
    } else {
        return rect_info.minification_filter;
    }
}

fn normalize_range(sampled_value: vec4f) -> vec4f {
    let range = rect_info.range_min_max;
    return (sampled_value - range.x) / (range.y - range.x);
}

fn decode_color(sampled_value: vec4f) -> vec4f {
    // Normalize the value first, otherwise premultiplying alpha and linear space conversion won't make sense.
    var rgba = normalize_range(sampled_value);

    // Convert to linear space
    if rect_info.decode_srgb != 0u {
        if all(vec3f(0.0) <= rgba.rgb) && all(rgba.rgb <= vec3f(1.0)) {
            rgba = linear_from_srgba(rgba);
        } else {
            rgba = ERROR_RGBA; // out of range
        }
    }

    // Premultiply alpha.
    if rect_info.multiply_rgb_with_alpha != 0u {
        rgba = vec4f(rgba.xyz * rgba.a, rgba.a);
    }

    return rgba;
}

/// Takes a floating point texel coordinate and outputs a integer texel coordinate
/// on the neighrest neighbor, clamped to the texture edge.
fn clamp_to_edge_nearest_neighbor(coord: vec2f, texture_dimension: vec2f) -> vec2i {
    return vec2i(clamp(floor(coord), vec2f(0.0), texture_dimension - vec2f(1.0)));
}

fn decode_color_and_filter_nearest_or_bilinear(filter_nearest: bool, coord: vec2f, v00: vec4f, v01: vec4f, v10: vec4f, v11: vec4f) -> vec4f {
    let c00 = decode_color(v00);
    if filter_nearest {
        return c00;
    } else {
        let c01 = decode_color(v01);
        let c10 = decode_color(v10);
        let c11 = decode_color(v11);
        let top = mix(c00, c10, fract(coord.x - 0.5));
        let bottom = mix(c01, c11, fract(coord.x - 0.5));
        return mix(top, bottom, fract(coord.y - 0.5));
    }
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4f {
    // Sample the main texture:
    var normalized_value: vec4f;
    var v00_coord: vec2i;
    var v01_coord: vec2i;
    var v10_coord: vec2i;
    var v11_coord: vec2i;

    var texture_dimensions: vec2f;
    if rect_info.sample_type == SAMPLE_TYPE_FLOAT {
        texture_dimensions = vec2f(textureDimensions(texture_float).xy);
    } else if rect_info.sample_type == SAMPLE_TYPE_SINT {
        texture_dimensions = vec2f(textureDimensions(texture_sint).xy);
    } else if rect_info.sample_type == SAMPLE_TYPE_UINT {
        texture_dimensions = vec2f(textureDimensions(texture_uint).xy);
    } else if rect_info.sample_type == SAMPLE_TYPE_NV12 {
        texture_dimensions = vec2f(textureDimensions(texture_uint).xy);
    } else if rect_info.sample_type == SAMPLE_TYPE_YUY2 {
        texture_dimensions = vec2f(textureDimensions(texture_uint).xy);
    }

    let coord = in.texcoord * texture_dimensions;

    let filter_nearest = (tex_filter(coord) == FILTER_NEAREST);

    if filter_nearest {
        v00_coord = clamp_to_edge_nearest_neighbor(coord, texture_dimensions);
        v01_coord = v00_coord;
        v10_coord = v00_coord;
        v11_coord = v00_coord;
    } else {
        v00_coord = clamp_to_edge_nearest_neighbor(coord + vec2f(-0.5, -0.5), texture_dimensions);
        v01_coord = clamp_to_edge_nearest_neighbor(coord + vec2f(-0.5, 0.5), texture_dimensions);
        v10_coord = clamp_to_edge_nearest_neighbor(coord + vec2f(0.5, -0.5), texture_dimensions);
        v11_coord = clamp_to_edge_nearest_neighbor(coord + vec2f(0.5, 0.5), texture_dimensions);
    }

    // WARNING! WARNING! WARNING! WARNING! WARNING! WARNING! WARNING! WARNING! WARNING! WARNING!
    // NO MORE SAMPLE TYPES CAN BE ADDED TO THIS SHADER!
    // The shader is already too large and adding more sample types will push us over the size limit.
    // See: https://github.com/rerun-io/rerun/issues/3931, https://github.com/rerun-io/rerun/issues/5073
    //
    // Note, in all the below branches we load the texture for all coords, even if we aren't doing to
    // use them. This avoids a branch to avoid running afoul of the size constraints in the above
    // bug. However, all coords were set to the same value above and so we should generally be hitting
    // the texture cache making this not quite as awful as it may appear.
    if rect_info.sample_type == SAMPLE_TYPE_FLOAT {
        normalized_value = decode_color_and_filter_nearest_or_bilinear(
            filter_nearest,
            coord,
            textureLoad(texture_float, v00_coord, 0),
            textureLoad(texture_float, v01_coord, 0),
            textureLoad(texture_float, v10_coord, 0),
            textureLoad(texture_float, v11_coord, 0));
    } else if rect_info.sample_type == SAMPLE_TYPE_SINT {
        normalized_value = decode_color_and_filter_nearest_or_bilinear(
            filter_nearest,
            coord,
            vec4f(textureLoad(texture_sint, v00_coord, 0)),
            vec4f(textureLoad(texture_sint, v01_coord, 0)),
            vec4f(textureLoad(texture_sint, v10_coord, 0)),
            vec4f(textureLoad(texture_sint, v11_coord, 0)));
    } else if rect_info.sample_type == SAMPLE_TYPE_UINT {
        normalized_value = decode_color_and_filter_nearest_or_bilinear(
            filter_nearest,
            coord,
            vec4f(textureLoad(texture_uint, v00_coord, 0)),
            vec4f(textureLoad(texture_uint, v01_coord, 0)),
            vec4f(textureLoad(texture_uint, v10_coord, 0)),
            vec4f(textureLoad(texture_uint, v11_coord, 0)));
    } else if rect_info.sample_type == SAMPLE_TYPE_NV12 || rect_info.sample_type == SAMPLE_TYPE_YUY2{
        normalized_value = decode_color_and_filter_nearest_or_bilinear(
            filter_nearest,
            coord,
            decode_nv12_or_yuy2(rect_info.sample_type, texture_uint, v00_coord),
            decode_nv12_or_yuy2(rect_info.sample_type, texture_uint, v01_coord),
            decode_nv12_or_yuy2(rect_info.sample_type, texture_uint, v10_coord),
            decode_nv12_or_yuy2(rect_info.sample_type, texture_uint, v11_coord));
    } else {
        return ERROR_RGBA; // unknown sample type
    }
    // WARNING! WARNING! WARNING! WARNING! WARNING! WARNING! WARNING! WARNING! WARNING! WARNING!

    // Apply gamma:
    normalized_value = vec4f(pow(normalized_value.rgb, vec3f(rect_info.gamma)), normalized_value.a);

    // Apply colormap, if any:
    var texture_color: vec4f;
    if rect_info.color_mapper == COLOR_MAPPER_OFF_GRAYSCALE {
        texture_color = vec4f(normalized_value.rrr, 1.0);
    } else if rect_info.color_mapper == COLOR_MAPPER_OFF_RGB {
        texture_color = normalized_value;
    } else if rect_info.color_mapper == COLOR_MAPPER_FUNCTION {
        let rgb = colormap_linear(rect_info.colormap_function, normalized_value.r);
        texture_color = vec4f(rgb, 1.0);
    } else if rect_info.color_mapper == COLOR_MAPPER_TEXTURE {
        let colormap_size = textureDimensions(colormap_texture).xy;
        let color_index = normalized_value.r * f32(colormap_size.x * colormap_size.y);
        // TODO(emilk): interpolate between neighboring colors for non-integral color indices
        // It's important to round here since otherwise numerical instability can push us to the adjacent class-id
        // See: https://github.com/rerun-io/rerun/issues/1968
        let color_index_u32 = u32(round(color_index));
        let x = color_index_u32 % colormap_size.x;
        let y = color_index_u32 / colormap_size.x;
        texture_color = textureLoad(colormap_texture, vec2u(x, y), 0);
    } else {
        return ERROR_RGBA; // unknown color mapper
    }

    return texture_color * rect_info.multiplicative_tint;
}

@fragment
fn fs_main_picking_layer(in: VertexOut) -> @location(0) vec4u {
    return vec4u(0u, 0u, 0u, 0u); // TODO(andreas): Implement picking layer id pass-through.
}

@fragment
fn fs_main_outline_mask(in: VertexOut) -> @location(0) vec2u {
    return rect_info.outline_mask;
}
