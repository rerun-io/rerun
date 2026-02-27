"""
Utility functions for compiling Slang shaders to WGSL.

This module provides helpers for compiling Slang (.slang) shader files into
WGSL source code and extracting reflection metadata as JSON. Requires either
`slangpy` (Python bindings) or the `slangc` command-line compiler to be
installed.

This is an experimental feature.
"""

from __future__ import annotations

import json
import subprocess
from pathlib import Path


def compile_slang_to_wgsl(source_path: str | Path) -> tuple[str, str]:
    """
    Compile a Slang shader file to WGSL and extract reflection JSON.

    Tries `slangpy` first, then falls back to shelling out to `slangc`.

    Parameters
    ----------
    source_path:
        Path to the `.slang` source file.

    Returns
    -------
    tuple[str, str]
        A tuple of (wgsl_source, parameters_json).
        `wgsl_source` is the compiled WGSL shader code.
        `parameters_json` is a JSON string describing shader parameters.

    Raises
    ------
    RuntimeError
        If neither `slangpy` nor `slangc` is available, or compilation fails.

    """
    source_path = Path(source_path)
    if not source_path.exists():
        raise FileNotFoundError(f"Slang source file not found: {source_path}")

    try:
        return _compile_with_slangpy(source_path)
    except ImportError:
        pass

    try:
        return _compile_with_slangc(source_path)
    except FileNotFoundError:
        pass

    raise RuntimeError(
        "Neither `slangpy` nor `slangc` is available. "
        "Install slangpy (`pip install slangpy`) or the Slang compiler (https://shader-slang.com)."
    )


def _compile_with_slangpy(source_path: Path) -> tuple[str, str]:
    """Compile using the slangpy Python bindings."""
    import slangpy  # type: ignore[import-not-found]

    module = slangpy.loadModule(str(source_path))

    wgsl_source = module.toWGSL()

    reflection = module.layout
    params_meta = _extract_reflection_metadata(reflection)
    parameters_json = json.dumps(params_meta, indent=2)

    return wgsl_source, parameters_json


def _compile_with_slangc(source_path: Path) -> tuple[str, str]:
    """Compile by shelling out to the slangc command-line compiler."""
    # Compile to WGSL
    try:
        wgsl_result = subprocess.run(
            ["slangc", str(source_path), "-target", "wgsl", "-o", "-"],
            capture_output=True,
            text=True,
            check=True,
        )
    except FileNotFoundError:
        raise
    except subprocess.CalledProcessError as e:
        raise RuntimeError(f"slangc compilation failed:\n{e.stderr}") from e

    wgsl_source = wgsl_result.stdout

    # Extract reflection as JSON
    try:
        reflect_result = subprocess.run(
            ["slangc", str(source_path), "-target", "wgsl", "-dump-reflection-json"],
            capture_output=True,
            text=True,
            check=True,
        )
        parameters_json = _parse_slangc_reflection(reflect_result.stdout)
    except subprocess.CalledProcessError as e:
        import warnings

        warnings.warn(
            f"slangc reflection extraction failed (stderr: {e.stderr.strip()}); "
            "using empty parameters",
            stacklevel=2,
        )
        parameters_json = json.dumps({"uniforms": [], "textures": []})
    except (json.JSONDecodeError, KeyError, ValueError) as e:
        import warnings

        warnings.warn(
            f"Failed to parse slangc reflection output: {e}; using empty parameters",
            stacklevel=2,
        )
        parameters_json = json.dumps({"uniforms": [], "textures": []})

    return wgsl_source, parameters_json


def _extract_reflection_metadata(reflection: object) -> dict:
    """
    Extract parameter metadata from slangpy reflection data.

    Converts slangpy's reflection layout into our JSON schema format.
    Texture bindings start at 1 to avoid conflict with the uniform buffer at binding 0.
    Samplers are auto-assigned at texture binding + 100.
    """
    uniforms: list[dict] = []
    textures: list[dict] = []

    if hasattr(reflection, "parameters"):
        for param in reflection.parameters:
            type_name = str(param.type) if hasattr(param, "type") else "float"
            name = str(param.name) if hasattr(param, "name") else ""

            if "Texture2D" in type_name:
                # Binding starts at 1 (binding 0 is reserved for the uniform buffer)
                textures.append({
                    "name": name,
                    "type": "texture_2d",
                    "binding": len(textures) + 1,
                    "source": f"./{name}",
                })
            elif "Texture3D" in type_name:
                textures.append({
                    "name": name,
                    "type": "texture_3d",
                    "binding": len(textures) + 1,
                    "source": f"./{name}",
                })
            else:
                param_type = _slang_type_to_param_type(type_name)
                uniforms.append({
                    "name": name,
                    "type": param_type,
                    "source": f"./{name}",
                })

    return {"uniforms": uniforms, "textures": textures}


def _parse_slangc_reflection(json_str: str) -> str:
    """Parse slangc reflection JSON output into our parameter format."""
    try:
        reflection = json.loads(json_str)
    except json.JSONDecodeError:
        return json.dumps({"uniforms": [], "textures": []})

    uniforms: list[dict] = []
    textures: list[dict] = []

    for param in reflection.get("parameters", []):
        name = param.get("name", "")
        type_info = param.get("type", {})
        kind = type_info.get("kind", "")

        if kind == "resource" and "Texture2D" in type_info.get("baseType", ""):
            # Binding starts at 1 (binding 0 is reserved for the uniform buffer)
            textures.append({
                "name": name,
                "type": "texture_2d",
                "binding": len(textures) + 1,
                "source": f"./{name}",
            })
        elif kind == "resource" and "Texture3D" in type_info.get("baseType", ""):
            textures.append({
                "name": name,
                "type": "texture_3d",
                "binding": len(textures) + 1,
                "source": f"./{name}",
            })
        else:
            param_type = _slang_type_to_param_type(kind)
            uniforms.append({
                "name": name,
                "type": param_type,
                "source": f"./{name}",
            })

    return json.dumps({"uniforms": uniforms, "textures": textures}, indent=2)


def _slang_type_to_param_type(slang_type: str) -> str:
    """Map a Slang type name to our parameter type string."""
    slang_type_lower = slang_type.lower()
    type_map = {
        "float": "float",
        "float2": "vec2",
        "float3": "vec3",
        "float4": "vec4",
        "vec2": "vec2",
        "vec3": "vec3",
        "vec4": "vec4",
        "float4x4": "mat4",
        "matrix": "mat4",
    }
    return type_map.get(slang_type_lower, "float")
