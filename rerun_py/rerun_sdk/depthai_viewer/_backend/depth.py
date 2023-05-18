ctrl = {
    "algorithm_control": {
        "align": "RECTIFIED_RIGHT",  # | 'RECTIFIED_LEFT' | 'CENTER'
        "unit": "METER",  # | 'CENTIMETER' | 'MILLIMETER' | 'INCH' | 'FOOT' | 'CUSTOM'
        "unit_multiplier": 1000,  # Only if 'unit' is 'CUSTOM'
        "lr_check": True,  # Enable left-right check
        "extended": True,  # Enable extended disparity
        "subpixel": True,  # Enable subpixel disparity
        "lr_check_threshold": 10,  # Left-right check threshold
        "subpixel_bits": 3,  # 3 | 4 | 5
        "disparity_shift": 0,  # Disparity shift
        "invalidate_edge_pixels": 0,  # Number of pixels to invalidate at the edge of the image
    },
    "postprocessing": {
        "median": 5,  # 0 | 3 | 5 | 7
        "bilateral_sigma": 0,  # Sigma value for bilateral filter
        "spatial": {
            "enable": True,  # Enable spatial denoise
            "hole_filling": 2,  # Hole filling radius
            "alpha": 0.5,  # Alpha factor in an exponential moving average
            "delta": 0,  # Step-size boundary
            "iterations": 1,  # Number of iterations
        },
        "temporal": {
            "enable": False,  # Enable or disable temporal denoise
            "persistency_mode": 3,  # Persistency mode (use corresponding integer value for the enum member)
            "alpha": 0.4,  # Alpha factor in an exponential moving average
            "delta": 0,  # Step-size boundary
        },
        "threshold": {
            "min_range": 0,  # Minimum range in depth units
            "max_range": 65535,  # Maximum range in depth units
        },
        "brightness": {
            "min": 0,  # Minimum pixel brightness
            "max": 256,  # Maximum pixel brightness
        },
        "speckle": {
            "enable": False,  # Enable or disable the speckle filter
            "range": 50,  # Speckle search range
        },
        "decimation": {
            "factor": 1,  # Decimation factor (1, 2, 3, or 4)
            "mode": 0,  # Decimation algorithm type (use corresponding integer value for the enum member)
        },
    },
    "census_transform": {
        "kernel_size": "AUTO",  # | 'KERNEL_5x5' | 'KERNEL_7x7' | 'KERNEL_7x9'
        "kernel_mask": 0,  # Census transform mask
        "enable_mean_mode": True,  # Enable mean mode
        "threshold": 0,  # Census transform comparison threshold value
    },
    "cost_matching": {
        "disparity_width": "DISPARITY_64",  # or 'DISPARITY_96'
        "enable_companding": False,  # Enable disparity companding using sparse matching
        "confidence_threshold": 245,  # Confidence threshold for accepted disparities
        "linear_equation_parameters": {
            "alpha": 0,
            "beta": 2,
            "threshold": 127,
        },
    },
    "cost_aggregation": {
        "division_factor": 1,  # Division factor for cost calculation linear equation parameters
        "horizontal_penalty_cost_p1": 250,  # Horizontal P1 penalty cost parameter
        "horizontal_penalty_cost_p2": 500,  # Horizontal P2 penalty cost parameter
        "vertical_penalty_cost_p1": 250,  # Vertical P1 penalty cost parameter
        "vertical_penalty_cost_p2": 500,  # Vertical P2 penalty cost parameter
    },
    "reset": False,  # Reset all controls to default
}
