#import <./rectangle.wgsl>

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

@fragment
fn fs_main(in: VertexOut) -> @location(0) Vec4 {
    // Sample the main texture:
    var sampled_value: Vec4;
    if rect_info.sample_type == SAMPLE_TYPE_FLOAT_FILTER {
        // TODO(emilk): support mipmaps
        sampled_value = textureSampleLevel(texture_float_filterable, texture_sampler, in.texcoord, 0.0);
    } else if rect_info.sample_type == SAMPLE_TYPE_FLOAT_NOFILTER {
        let coord = in.texcoord * Vec2(textureDimensions(texture_float).xy);
        if tex_filter(coord) == FILTER_NEAREST {
            // nearest
            sampled_value = textureLoad(texture_float, IVec2(coord + vec2(0.5)), 0);
        } else {
            // bilinear
            let v00 = textureLoad(texture_float, IVec2(coord) + IVec2(0, 0), 0);
            let v01 = textureLoad(texture_float, IVec2(coord) + IVec2(0, 1), 0);
            let v10 = textureLoad(texture_float, IVec2(coord) + IVec2(1, 0), 0);
            let v11 = textureLoad(texture_float, IVec2(coord) + IVec2(1, 1), 0);
            let top = mix(v00, v10, fract(coord.x));
            let bottom = mix(v01, v11, fract(coord.x));
            sampled_value = mix(top, bottom, fract(coord.y));
        }
    } else if rect_info.sample_type == SAMPLE_TYPE_SINT_NOFILTER {
        let coord = in.texcoord * Vec2(textureDimensions(texture_sint).xy);
        if tex_filter(coord) == FILTER_NEAREST {
            // nearest
            sampled_value = Vec4(textureLoad(texture_sint, IVec2(coord + vec2(0.5)), 0));
        } else {
            // bilinear
            let v00 = Vec4(textureLoad(texture_sint, IVec2(coord) + IVec2(0, 0), 0));
            let v01 = Vec4(textureLoad(texture_sint, IVec2(coord) + IVec2(0, 1), 0));
            let v10 = Vec4(textureLoad(texture_sint, IVec2(coord) + IVec2(1, 0), 0));
            let v11 = Vec4(textureLoad(texture_sint, IVec2(coord) + IVec2(1, 1), 0));
            let top = mix(v00, v10, fract(coord.x));
            let bottom = mix(v01, v11, fract(coord.x));
            sampled_value = mix(top, bottom, fract(coord.y));
        }
    } else if rect_info.sample_type == SAMPLE_TYPE_UINT_NOFILTER {
        let coord = in.texcoord * Vec2(textureDimensions(texture_uint).xy);
        if tex_filter(coord) == FILTER_NEAREST {
            // nearest
            sampled_value = Vec4(textureLoad(texture_uint, IVec2(coord + vec2(0.5)), 0));
        } else {
            // bilinear
            let v00 = Vec4(textureLoad(texture_uint, IVec2(coord) + IVec2(0, 0), 0));
            let v01 = Vec4(textureLoad(texture_uint, IVec2(coord) + IVec2(0, 1), 0));
            let v10 = Vec4(textureLoad(texture_uint, IVec2(coord) + IVec2(1, 0), 0));
            let v11 = Vec4(textureLoad(texture_uint, IVec2(coord) + IVec2(1, 1), 0));
            let top = mix(v00, v10, fract(coord.x));
            let bottom = mix(v01, v11, fract(coord.x));
            sampled_value = mix(top, bottom, fract(coord.y));
        }
    } else {
        return ERROR_RGBA; // unknown sample type
    }

    // Normalize the sample:
    let range = rect_info.range_min_max;
    var normalized_value: Vec4 = (sampled_value - range.x) / (range.y - range.x);

    // Apply gamma:
    normalized_value = vec4(pow(normalized_value.rgb, vec3(rect_info.gamma)), normalized_value.a); // TODO(emilk): handle premultiplied alpha

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
        let color_index_i32 = i32(color_index);
        let x = color_index_i32 % colormap_size.x;
        let y = color_index_i32 / colormap_size.x;
        texture_color = textureLoad(colormap_texture, IVec2(x, y), 0);
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
