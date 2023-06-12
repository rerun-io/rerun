#import <./colormap.wgsl>
#import <./rectangle.wgsl>
#import <./utils/srgb.wgsl>

fn is_magnifying(pixel_coord: Vec2) -> bool {
    return fwidth(pixel_coord.x) < 1.0;
}

fn tex_filter(pixel_coord: Vec2) -> u32 {
    if is_magnifying(pixel_coord) {
        return rect_info.magnification_filter;
    } else {
        return rect_info.minification_filter;
    }
}

fn normalize_range(sampled_value: Vec4) -> Vec4 {
    let range = rect_info.range_min_max;
    return (sampled_value - range.x) / (range.y - range.x);
}

fn decode_color(sampled_value: Vec4) -> Vec4 {
    // Normalize the value first, otherwise premultiplying alpha and linear space conversion won't make sense.
    var rgba = normalize_range(sampled_value);

    // Convert to linear space
    if rect_info.decode_srgb != 0u {
        if all(0.0 <= rgba.rgb) && all(rgba.rgb <= 1.0) {
            rgba = linear_from_srgba(rgba);
        } else {
            rgba = ERROR_RGBA; // out of range
        }
    }

    // Premultiply alpha.
    if rect_info.multiply_rgb_with_alpha != 0u {
        rgba = vec4(rgba.xyz * rgba.a, rgba.a);
    }

    return rgba;
}

/// Takes a floating point texel coordinate and outputs a neighrest neighbor
fn to_clamp_to_border_sample(coord: Vec2, texture_dimension: Vec2) -> IVec2 {
    return IVec2(clamp(floor(coord), Vec2(0.0), texture_dimension - Vec2(1.0)));
}

/// Takes a floating point texel coordinate and outputs the four integer texel coordinates that are used for bilinear filtering.
/// All four samples are clamped to the texture border.
fn clamped_bilinear_sample_positions(coord: Vec2, texture_dimension: Vec2) -> array<IVec2, 4> {
    let v00 = to_clamp_to_border_sample(coord + vec2(-0.5, -0.5), texture_dimension);
    let v01 = to_clamp_to_border_sample(coord + vec2(-0.5,  0.5), texture_dimension);
    let v10 = to_clamp_to_border_sample(coord + vec2( 0.5, -0.5), texture_dimension);
    let v11 = to_clamp_to_border_sample(coord + vec2( 0.5,  0.5), texture_dimension);
    return array<IVec2, 4>(IVec2(v00), IVec2(v01), IVec2(v10), IVec2(v11));
}

fn filter_bilinear(coord: Vec2, v00: Vec4, v01: Vec4, v10: Vec4, v11: Vec4) -> Vec4 {
    let top = mix(v00, v10, fract(coord.x - 0.5 + 10.0));
    let bottom = mix(v01, v11, fract(coord.x - 0.5 + 10.0));
    return mix(top, bottom, fract(coord.y - 0.5 + 10.0));
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) Vec4 {
    // Sample the main texture:
    var normalized_value: Vec4;
    if rect_info.sample_type == SAMPLE_TYPE_FLOAT {
        let texture_dimensions = Vec2(textureDimensions(texture_float).xy);
        let coord = in.texcoord * texture_dimensions;
        if tex_filter(coord) == FILTER_NEAREST {
            // nearest
            normalized_value = decode_color(textureLoad(texture_float,
                to_clamp_to_border_sample(coord, texture_dimensions), 0));
        } else {
            // bilinear
            let sample_positions = clamped_bilinear_sample_positions(coord, Vec2(textureDimensions(texture_float).xy));
            let v00 = decode_color(textureLoad(texture_float, sample_positions[0], 0));
            let v01 = decode_color(textureLoad(texture_float, sample_positions[1], 0));
            let v10 = decode_color(textureLoad(texture_float, sample_positions[2], 0));
            let v11 = decode_color(textureLoad(texture_float, sample_positions[3], 0));
            normalized_value = filter_bilinear(coord, v00, v01, v10, v11);
        }
    } else if rect_info.sample_type == SAMPLE_TYPE_SINT {
        let texture_dimensions = Vec2(textureDimensions(texture_sint).xy);
        let coord = in.texcoord * texture_dimensions;
        if tex_filter(coord) == FILTER_NEAREST {
            // nearest
            normalized_value = decode_color(Vec4(textureLoad(texture_sint,
                to_clamp_to_border_sample(coord, texture_dimensions), 0)));
        } else {
            // bilinear
            let sample_positions = clamped_bilinear_sample_positions(coord, Vec2(textureDimensions(texture_float).xy));
            let v00 = decode_color(Vec4(textureLoad(texture_sint, sample_positions[0], 0)));
            let v01 = decode_color(Vec4(textureLoad(texture_sint, sample_positions[1], 0)));
            let v10 = decode_color(Vec4(textureLoad(texture_sint, sample_positions[2], 0)));
            let v11 = decode_color(Vec4(textureLoad(texture_sint, sample_positions[3], 0)));
            normalized_value = filter_bilinear(coord, v00, v01, v10, v11);
        }
    } else if rect_info.sample_type == SAMPLE_TYPE_UINT {
        let texture_dimensions = Vec2(textureDimensions(texture_uint).xy);
        let coord = in.texcoord * texture_dimensions;
        if tex_filter(coord) == FILTER_NEAREST {
            // nearest
            normalized_value = decode_color(Vec4(textureLoad(texture_uint,
                to_clamp_to_border_sample(coord, texture_dimensions), 0)));
        } else {
            // bilinear
            let sample_positions = clamped_bilinear_sample_positions(coord, Vec2(textureDimensions(texture_float).xy));
            let v00 = decode_color(Vec4(textureLoad(texture_uint, sample_positions[0], 0)));
            let v01 = decode_color(Vec4(textureLoad(texture_uint, sample_positions[1], 0)));
            let v10 = decode_color(Vec4(textureLoad(texture_uint, sample_positions[2], 0)));
            let v11 = decode_color(Vec4(textureLoad(texture_uint, sample_positions[3], 0)));
            normalized_value = filter_bilinear(coord, v00, v01, v10, v11);
        }
    } else {
        return ERROR_RGBA; // unknown sample type
    }

    // Apply gamma:
    normalized_value = vec4(pow(normalized_value.rgb, vec3(rect_info.gamma)), normalized_value.a);

    // Apply colormap, if any:
    var texture_color: Vec4;
    if rect_info.color_mapper == COLOR_MAPPER_OFF {
        texture_color = normalized_value;
    } else if rect_info.color_mapper == COLOR_MAPPER_FUNCTION {
        let rgb = colormap_linear(rect_info.colormap_function, normalized_value.r);
        texture_color = Vec4(rgb, 1.0);
    } else if rect_info.color_mapper == COLOR_MAPPER_TEXTURE {
        let colormap_size = textureDimensions(colormap_texture).xy;
        let color_index = normalized_value.r * f32(colormap_size.x * colormap_size.y);
        // TODO(emilk): interpolate between neighboring colors for non-integral color indices
        // It's important to round here since otherwise numerical instability can push us to the adjacent class-id
        // See: https://github.com/rerun-io/rerun/issues/1968
        let color_index_u32 = u32(round(color_index));
        let x = color_index_u32 % colormap_size.x;
        let y = color_index_u32 / colormap_size.x;
        texture_color = textureLoad(colormap_texture, UVec2(x, y), 0);
    } else {
        return ERROR_RGBA; // unknown color mapper
    }

    return texture_color * rect_info.multiplicative_tint;
}

@fragment
fn fs_main_picking_layer(in: VertexOut) -> @location(0) UVec4 {
    return UVec4(0u, 0u, 0u, 0u); // TODO(andreas): Implement picking layer id pass-through.
}

@fragment
fn fs_main_outline_mask(in: VertexOut) -> @location(0) UVec2 {
    return rect_info.outline_mask;
}
