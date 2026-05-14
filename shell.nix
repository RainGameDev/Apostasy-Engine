{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    rustc
    cargo
    rust-analyzer
    gcc
    cmake
    ninja
    pkg-config
    mold

    # Shader / Vulkan
    shaderc
    glslang
    spirv-tools
    vulkan-loader
    vulkan-headers
    vulkan-validation-layers

    # Wayland
    wayland
    wayland-protocols
    libxkbcommon

    # DBus 
    dbus

    # Fonts 
    fontconfig
    freetype

    # Clipboard 
    libGL

    # openssl 
    openssl
    openssl.dev
  ];

  WAYLAND_DISPLAY = "wayland-1";

  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (with pkgs; [
    wayland
    libxkbcommon
    vulkan-loader
    libGL
    dbus
  ]);

  SHADERC_LIB_DIR = "${pkgs.shaderc.lib}/lib";
  SHADERC_SKIP_BUILD = "1";

  # Helps pkg-config find openssl
  PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
}
