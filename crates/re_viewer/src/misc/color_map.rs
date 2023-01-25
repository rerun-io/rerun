const TURBO_SRGB_BYTES: [[u8; 3]; 256] = [
    [48, 18, 59],
    [50, 21, 67],
    [51, 24, 74],
    [52, 27, 81],
    [53, 30, 88],
    [54, 33, 95],
    [55, 36, 102],
    [56, 39, 109],
    [57, 42, 115],
    [58, 45, 121],
    [59, 47, 128],
    [60, 50, 134],
    [61, 53, 139],
    [62, 56, 145],
    [63, 59, 151],
    [63, 62, 156],
    [64, 64, 162],
    [65, 67, 167],
    [65, 70, 172],
    [66, 73, 177],
    [66, 75, 181],
    [67, 78, 186],
    [68, 81, 191],
    [68, 84, 195],
    [68, 86, 199],
    [69, 89, 203],
    [69, 92, 207],
    [69, 94, 211],
    [70, 97, 214],
    [70, 100, 218],
    [70, 102, 221],
    [70, 105, 224],
    [70, 107, 227],
    [71, 110, 230],
    [71, 113, 233],
    [71, 115, 235],
    [71, 118, 238],
    [71, 120, 240],
    [71, 123, 242],
    [70, 125, 244],
    [70, 128, 246],
    [70, 130, 248],
    [70, 133, 250],
    [70, 135, 251],
    [69, 138, 252],
    [69, 140, 253],
    [68, 143, 254],
    [67, 145, 254],
    [66, 148, 255],
    [65, 150, 255],
    [64, 153, 255],
    [62, 155, 254],
    [61, 158, 254],
    [59, 160, 253],
    [58, 163, 252],
    [56, 165, 251],
    [55, 168, 250],
    [53, 171, 248],
    [51, 173, 247],
    [49, 175, 245],
    [47, 178, 244],
    [46, 180, 242],
    [44, 183, 240],
    [42, 185, 238],
    [40, 188, 235],
    [39, 190, 233],
    [37, 192, 231],
    [35, 195, 228],
    [34, 197, 226],
    [32, 199, 223],
    [31, 201, 221],
    [30, 203, 218],
    [28, 205, 216],
    [27, 208, 213],
    [26, 210, 210],
    [26, 212, 208],
    [25, 213, 205],
    [24, 215, 202],
    [24, 217, 200],
    [24, 219, 197],
    [24, 221, 194],
    [24, 222, 192],
    [24, 224, 189],
    [25, 226, 187],
    [25, 227, 185],
    [26, 228, 182],
    [28, 230, 180],
    [29, 231, 178],
    [31, 233, 175],
    [32, 234, 172],
    [34, 235, 170],
    [37, 236, 167],
    [39, 238, 164],
    [42, 239, 161],
    [44, 240, 158],
    [47, 241, 155],
    [50, 242, 152],
    [53, 243, 148],
    [56, 244, 145],
    [60, 245, 142],
    [63, 246, 138],
    [67, 247, 135],
    [70, 248, 132],
    [74, 248, 128],
    [78, 249, 125],
    [82, 250, 122],
    [85, 250, 118],
    [89, 251, 115],
    [93, 252, 111],
    [97, 252, 108],
    [101, 253, 105],
    [105, 253, 102],
    [109, 254, 98],
    [113, 254, 95],
    [117, 254, 92],
    [121, 254, 89],
    [125, 255, 86],
    [128, 255, 83],
    [132, 255, 81],
    [136, 255, 78],
    [139, 255, 75],
    [143, 255, 73],
    [146, 255, 71],
    [150, 254, 68],
    [153, 254, 66],
    [156, 254, 64],
    [159, 253, 63],
    [161, 253, 61],
    [164, 252, 60],
    [167, 252, 58],
    [169, 251, 57],
    [172, 251, 56],
    [175, 250, 55],
    [177, 249, 54],
    [180, 248, 54],
    [183, 247, 53],
    [185, 246, 53],
    [188, 245, 52],
    [190, 244, 52],
    [193, 243, 52],
    [195, 241, 52],
    [198, 240, 52],
    [200, 239, 52],
    [203, 237, 52],
    [205, 236, 52],
    [208, 234, 52],
    [210, 233, 53],
    [212, 231, 53],
    [215, 229, 53],
    [217, 228, 54],
    [219, 226, 54],
    [221, 224, 55],
    [223, 223, 55],
    [225, 221, 55],
    [227, 219, 56],
    [229, 217, 56],
    [231, 215, 57],
    [233, 213, 57],
    [235, 211, 57],
    [236, 209, 58],
    [238, 207, 58],
    [239, 205, 58],
    [241, 203, 58],
    [242, 201, 58],
    [244, 199, 58],
    [245, 197, 58],
    [246, 195, 58],
    [247, 193, 58],
    [248, 190, 57],
    [249, 188, 57],
    [250, 186, 57],
    [251, 184, 56],
    [251, 182, 55],
    [252, 179, 54],
    [252, 177, 54],
    [253, 174, 53],
    [253, 172, 52],
    [254, 169, 51],
    [254, 167, 50],
    [254, 164, 49],
    [254, 161, 48],
    [254, 158, 47],
    [254, 155, 45],
    [254, 153, 44],
    [254, 150, 43],
    [254, 147, 42],
    [254, 144, 41],
    [253, 141, 39],
    [253, 138, 38],
    [252, 135, 37],
    [252, 132, 35],
    [251, 129, 34],
    [251, 126, 33],
    [250, 123, 31],
    [249, 120, 30],
    [249, 117, 29],
    [248, 114, 28],
    [247, 111, 26],
    [246, 108, 25],
    [245, 105, 24],
    [244, 102, 23],
    [243, 99, 21],
    [242, 96, 20],
    [241, 93, 19],
    [240, 91, 18],
    [239, 88, 17],
    [237, 85, 16],
    [236, 83, 15],
    [235, 80, 14],
    [234, 78, 13],
    [232, 75, 12],
    [231, 73, 12],
    [229, 71, 11],
    [228, 69, 10],
    [226, 67, 10],
    [225, 65, 9],
    [223, 63, 8],
    [221, 61, 8],
    [220, 59, 7],
    [218, 57, 7],
    [216, 55, 6],
    [214, 53, 6],
    [212, 51, 5],
    [210, 49, 5],
    [208, 47, 5],
    [206, 45, 4],
    [204, 43, 4],
    [202, 42, 4],
    [200, 40, 3],
    [197, 38, 3],
    [195, 37, 3],
    [193, 35, 2],
    [190, 33, 2],
    [188, 32, 2],
    [185, 30, 2],
    [183, 29, 2],
    [180, 27, 1],
    [178, 26, 1],
    [175, 24, 1],
    [172, 23, 1],
    [169, 22, 1],
    [167, 20, 1],
    [164, 19, 1],
    [161, 18, 1],
    [158, 16, 1],
    [155, 15, 1],
    [152, 14, 1],
    [149, 13, 1],
    [146, 11, 1],
    [142, 10, 1],
    [139, 9, 2],
    [136, 8, 2],
    [133, 7, 2],
    [129, 6, 2],
    [126, 5, 2],
    [122, 4, 3],
];

/// Given a value in [0, 1], output `sRGB`.
#[inline]
pub fn turbo_color_map(t: f32) -> [u8; 3] {
    // TODO(emilk): interpolate, or use a polynomial approximation (https://gist.github.com/mikhailov-work/0d177465a8151eb6ede1768d51d476c7)
    let index = (t * 255.0 + 0.5) as usize;
    TURBO_SRGB_BYTES[index]
}

/// LUT as defined [here](https://github.com/sjmgarnier/viridisLite/blob/ffc7061/R/zzz.R)
///
/// Converted to bytes using this python snippet:
/// ```python
/// for (r, g, b, asdf) in zip(R, G, B, opt):
//      if asdf == 'D':
//          print("[{}, {}, {}],".format(int(r * 255.0 + 0.5), int(g * 255.0 + 0.5), int(b * 255.0 + 0.5)))
/// ```
const VIRIDIS_SRGB_BYTES: [[u8; 3]; 256] = [
    [68, 1, 84],
    [68, 2, 86],
    [69, 4, 87],
    [69, 5, 89],
    [70, 7, 90],
    [70, 8, 92],
    [70, 10, 93],
    [70, 11, 94],
    [71, 13, 96],
    [71, 14, 97],
    [71, 16, 99],
    [71, 17, 100],
    [71, 19, 101],
    [72, 20, 103],
    [72, 22, 104],
    [72, 23, 105],
    [72, 24, 106],
    [72, 26, 108],
    [72, 27, 109],
    [72, 28, 110],
    [72, 29, 111],
    [72, 31, 112],
    [72, 32, 113],
    [72, 33, 115],
    [72, 35, 116],
    [72, 36, 117],
    [72, 37, 118],
    [72, 38, 119],
    [72, 40, 120],
    [72, 41, 121],
    [71, 42, 122],
    [71, 44, 122],
    [71, 45, 123],
    [71, 46, 124],
    [71, 47, 125],
    [70, 48, 126],
    [70, 50, 126],
    [70, 51, 127],
    [70, 52, 128],
    [69, 53, 129],
    [69, 55, 129],
    [69, 56, 130],
    [68, 57, 131],
    [68, 58, 131],
    [68, 59, 132],
    [67, 61, 132],
    [67, 62, 133],
    [66, 63, 133],
    [66, 64, 134],
    [66, 65, 134],
    [65, 66, 135],
    [65, 68, 135],
    [64, 69, 136],
    [64, 70, 136],
    [63, 71, 136],
    [63, 72, 137],
    [62, 73, 137],
    [62, 74, 137],
    [62, 76, 138],
    [61, 77, 138],
    [61, 78, 138],
    [60, 79, 138],
    [60, 80, 139],
    [59, 81, 139],
    [59, 82, 139],
    [58, 83, 139],
    [58, 84, 140],
    [57, 85, 140],
    [57, 86, 140],
    [56, 88, 140],
    [56, 89, 140],
    [55, 90, 140],
    [55, 91, 141],
    [54, 92, 141],
    [54, 93, 141],
    [53, 94, 141],
    [53, 95, 141],
    [52, 96, 141],
    [52, 97, 141],
    [51, 98, 141],
    [51, 99, 141],
    [50, 100, 142],
    [50, 101, 142],
    [49, 102, 142],
    [49, 103, 142],
    [49, 104, 142],
    [48, 105, 142],
    [48, 106, 142],
    [47, 107, 142],
    [47, 108, 142],
    [46, 109, 142],
    [46, 110, 142],
    [46, 111, 142],
    [45, 112, 142],
    [45, 113, 142],
    [44, 113, 142],
    [44, 114, 142],
    [44, 115, 142],
    [43, 116, 142],
    [43, 117, 142],
    [42, 118, 142],
    [42, 119, 142],
    [42, 120, 142],
    [41, 121, 142],
    [41, 122, 142],
    [41, 123, 142],
    [40, 124, 142],
    [40, 125, 142],
    [39, 126, 142],
    [39, 127, 142],
    [39, 128, 142],
    [38, 129, 142],
    [38, 130, 142],
    [38, 130, 142],
    [37, 131, 142],
    [37, 132, 142],
    [37, 133, 142],
    [36, 134, 142],
    [36, 135, 142],
    [35, 136, 142],
    [35, 137, 142],
    [35, 138, 141],
    [34, 139, 141],
    [34, 140, 141],
    [34, 141, 141],
    [33, 142, 141],
    [33, 143, 141],
    [33, 144, 141],
    [33, 145, 140],
    [32, 146, 140],
    [32, 146, 140],
    [32, 147, 140],
    [31, 148, 140],
    [31, 149, 139],
    [31, 150, 139],
    [31, 151, 139],
    [31, 152, 139],
    [31, 153, 138],
    [31, 154, 138],
    [30, 155, 138],
    [30, 156, 137],
    [30, 157, 137],
    [31, 158, 137],
    [31, 159, 136],
    [31, 160, 136],
    [31, 161, 136],
    [31, 161, 135],
    [31, 162, 135],
    [32, 163, 134],
    [32, 164, 134],
    [33, 165, 133],
    [33, 166, 133],
    [34, 167, 133],
    [34, 168, 132],
    [35, 169, 131],
    [36, 170, 131],
    [37, 171, 130],
    [37, 172, 130],
    [38, 173, 129],
    [39, 173, 129],
    [40, 174, 128],
    [41, 175, 127],
    [42, 176, 127],
    [44, 177, 126],
    [45, 178, 125],
    [46, 179, 124],
    [47, 180, 124],
    [49, 181, 123],
    [50, 182, 122],
    [52, 182, 121],
    [53, 183, 121],
    [55, 184, 120],
    [56, 185, 119],
    [58, 186, 118],
    [59, 187, 117],
    [61, 188, 116],
    [63, 188, 115],
    [64, 189, 114],
    [66, 190, 113],
    [68, 191, 112],
    [70, 192, 111],
    [72, 193, 110],
    [74, 193, 109],
    [76, 194, 108],
    [78, 195, 107],
    [80, 196, 106],
    [82, 197, 105],
    [84, 197, 104],
    [86, 198, 103],
    [88, 199, 101],
    [90, 200, 100],
    [92, 200, 99],
    [94, 201, 98],
    [96, 202, 96],
    [99, 203, 95],
    [101, 203, 94],
    [103, 204, 92],
    [105, 205, 91],
    [108, 205, 90],
    [110, 206, 88],
    [112, 207, 87],
    [115, 208, 86],
    [117, 208, 84],
    [119, 209, 83],
    [122, 209, 81],
    [124, 210, 80],
    [127, 211, 78],
    [129, 211, 77],
    [132, 212, 75],
    [134, 213, 73],
    [137, 213, 72],
    [139, 214, 70],
    [142, 214, 69],
    [144, 215, 67],
    [147, 215, 65],
    [149, 216, 64],
    [152, 216, 62],
    [155, 217, 60],
    [157, 217, 59],
    [160, 218, 57],
    [162, 218, 55],
    [165, 219, 54],
    [168, 219, 52],
    [170, 220, 50],
    [173, 220, 48],
    [176, 221, 47],
    [178, 221, 45],
    [181, 222, 43],
    [184, 222, 41],
    [186, 222, 40],
    [189, 223, 38],
    [192, 223, 37],
    [194, 223, 35],
    [197, 224, 33],
    [200, 224, 32],
    [202, 225, 31],
    [205, 225, 29],
    [208, 225, 28],
    [210, 226, 27],
    [213, 226, 26],
    [216, 226, 25],
    [218, 227, 25],
    [221, 227, 24],
    [223, 227, 24],
    [226, 228, 24],
    [229, 228, 25],
    [231, 228, 25],
    [234, 229, 26],
    [236, 229, 27],
    [239, 229, 28],
    [241, 229, 29],
    [244, 230, 30],
    [246, 230, 32],
    [248, 230, 33],
    [251, 231, 35],
    [253, 231, 37],
];

/// Given a value in [0, 1], output `sRGB` in Viridis color map.
#[inline]
pub fn viridis_color_map(t: f32) -> [u8; 3] {
    // TODO(andreas): interpolate
    let index = (t * 255.0 + 0.5) as usize;
    VIRIDIS_SRGB_BYTES[index]
}
