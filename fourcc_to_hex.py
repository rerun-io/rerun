#!/usr/bin/env python3
"""
Convert 4-character codes (FourCC) to hexadecimal values.
Useful for assigning numeric values to codec enums based on standard identifiers.
"""


def fourcc_to_hex(fourcc: str, little_endian: bool = True) -> int:
    """
    Convert a 4-character code to its hexadecimal representation.

    Args:
        fourcc: 4-character string code
        little_endian: If True, return little-endian format; if False, big-endian

    Returns:
        Integer representation of the FourCC code
    """
    if len(fourcc) != 4:
        raise ValueError(f"FourCC must be exactly 4 characters, got {len(fourcc)}: '{fourcc}'")

    # Convert each character to its byte value
    bytes_values = [ord(c) for c in fourcc]

    if little_endian:
        # Little-endian: reverse the byte order
        bytes_values = bytes_values[::-1]

    # Combine bytes into a single integer
    result = 0
    for i, byte_val in enumerate(bytes_values):
        result |= byte_val << (8 * i)

    return result


def print_codec_values():
    """Print hex values for common video codec FourCC codes."""
    codecs = {
        "H264": "avc1",  # Standard FourCC for H.264
        "H265": "hvc1",  # Standard FourCC for H.265
        "VP8": "VP80",  # Standard FourCC for VP8
        "VP9": "VP90",  # Standard FourCC for VP9
        "AV1": "av01",  # Standard FourCC for AV1
    }

    print("Video Codec FourCC to Hex Conversion:")
    print("=" * 50)
    print(f"{'Codec':<8} {'FourCC':<8} {'Little-Endian':<15} {'Big-Endian':<15}")
    print("-" * 50)

    for codec_name, fourcc in codecs.items():
        little_endian = fourcc_to_hex(fourcc, little_endian=True)
        big_endian = fourcc_to_hex(fourcc, little_endian=False)

        print(
            f"{codec_name:<8} {fourcc:<8} 0x{little_endian:08X} ({little_endian:<8}) 0x{big_endian:08X} ({big_endian:<8})"
        )


if __name__ == "__main__":
    print_codec_values()

    print("\nCustom FourCC conversion:")
    print("=" * 30)

    # Example usage
    custom_fourcc = input("Enter a 4-character code (or press Enter to skip): ").strip()
    if custom_fourcc:
        try:
            le_value = fourcc_to_hex(custom_fourcc, little_endian=True)
            be_value = fourcc_to_hex(custom_fourcc, little_endian=False)
            print(f"'{custom_fourcc}' -> Little-Endian: 0x{le_value:08X} ({le_value})")
            print(f"'{custom_fourcc}' -> Big-Endian: 0x{be_value:08X} ({be_value})")
        except ValueError as e:
            print(f"Error: {e}")
