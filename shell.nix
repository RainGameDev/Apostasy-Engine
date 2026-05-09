{pkgs ? import <nixpkgs> {} }:

pkgs.mkShell{
  buildInputs = with pkgs; [
    rustc
    cargo
    rust-analyzer
    gcc
    cmake
    ninja
    pkg-config
    shaderc
    glslang
    spirv-tools
    vulkan-loader
    vulkan-headers
    vulkan-validation-layers
    wayland
    wayland-protocols
    libxkbcommon
  ];


        WAYLAND_DISPLAY = "wayland-1";
        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (with pkgs; [wayland libxkbcommon vulkan-loader]); 
  SHADERC_LIB_DIR = "${pkgs.shaderc.lib}/lib";
  SHADERC_SKIP_BUILD = "1";

}
