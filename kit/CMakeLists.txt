# bui_kit - Blender's interface as a retained-mode C runtime: nodes, layout,
# input handling (interface_handlers.cc rules) and widget drawing ported from
# interface_widgets.cc, rendered into a BuiRaster with damage tracking.
#
# SPDX-License-Identifier: GPL-2.0-or-later

add_library(bui_kit STATIC
  intern/kit_core.c
  intern/kit_draw.c
  intern/kit_theme_presets.c
  intern/kit_theme.c
  intern/kit_json.c
  intern/kit_icons_data.c
  intern/kit_widgets.c
  intern/kit_views.c
  intern/kit_menus.c
  intern/kit_color.c
  intern/kit_frame.c
  intern/kit_fling.c
  intern/kit_textarea.c
  intern/kit_ime.c
  intern/kit_ime_win32.c
  intern/kit_window_win32.c
  intern/kit_window_x11.c
  intern/kit_window_null.c
  intern/kit_icons_svg.c
  intern/kit_regions.c
  # screens, areas and regions (Blender's editors/screen): see intern/screen/screen_intern.h
  intern/screen/area.c
  intern/screen/screen_draw.c
  intern/screen/screen_edit.c
  intern/screen/screen_geometry.c
  intern/screen/screen_layout.c
  intern/screen/screen_node.c
  intern/screen/screen_ops.c
  intern/kit_nodes.c
  intern/kit_custom.c
  intern/kit_touch.c
  intern/kit_filebrowser.c # without BUI_FILEBROWSER: BK_file_dialog's stub only
  # images and video (Blender's imbuf design): see intern/image/image.h
  intern/image/imbuf.c
  intern/image/filetype.c
  intern/image/anim.c
  intern/image/play.c
  intern/image/format_svg.c
  intern/image/cache.c
  intern/image/loader.c
  intern/image/draw.c
  intern/image/video.c
  intern/image/movie.c
  # sound (intern/audio): the player and BK_sound_*; silent without a device backend below
  intern/audio/audio.c
  intern/audio/sound.c
  # Blender's UI sources copied and edited for the kit (C++; see blender/bk_blender.h)
  intern/blender/interface_string_search.cc
  intern/blender/interface_utils.cc
  intern/blender/interface_undo.cc
  extern/nanosvg/blender_nanosvg.c
  extern/nanosvg/blender_raster.c
)
# Blender's icons: its SVGs (kit_icons_svg.c) rasterized with its nanosvg.
target_include_directories(bui_kit PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/extern/nanosvg)
set_source_files_properties(extern/nanosvg/blender_nanosvg.c extern/nanosvg/blender_raster.c
                            PROPERTIES COMPILE_OPTIONS "$<IF:$<C_COMPILER_ID:MSVC>,/w,-w>")
add_library(bui::kit ALIAS bui_kit)
# the optional parts this build has (features.json): BUI_IMAGE_*, BUI_VIDEO*, ...
bui_features(bui_kit)

# Animation (GSAP- and Motion-style): the engine, or the API alone when it is off
if(BUI_ANIM)
  target_sources(bui_kit PRIVATE
    intern/anim/ease.c
    intern/anim/spring.c
    intern/anim/value.c
    intern/anim/motion.c
    intern/anim/node.c)
else()
  target_sources(bui_kit PRIVATE intern/anim/anim_off.c)
endif()

# Accessibility (intern/a11y): each context's tree, and the adapter that serves it to screen
# readers (bui-a11y, the Rust crate in platform/a11y/ with AccessKit); without it, the API alone.
if(BUI_A11Y)
  target_sources(bui_kit PRIVATE intern/a11y/a11y.c)
  target_include_directories(bui_kit PRIVATE ${BUI_SOURCE_DIR}/platform/a11y/include)
  # A Rust program links bui-a11y itself (blenderui-sys depends on the crate and turns this off);
  # C builds get it from cargo, as a static library.
  option(BUI_A11Y_CARGO "Build bui-a11y with cargo and link it (BUI_A11Y)" ON)
  if(BUI_A11Y_CARGO)
    find_program(BUI_CARGO cargo)
    if(NOT BUI_CARGO)
      message(FATAL_ERROR "BUI_A11Y needs cargo (Rust) to build platform/a11y/: install it, or -DBUI_A11Y=OFF")
    endif()
    set(_a11y_dir ${CMAKE_CURRENT_BINARY_DIR}/a11y)
    set(_a11y_lib ${_a11y_dir}/release/${CMAKE_STATIC_LIBRARY_PREFIX}bui_a11y${CMAKE_STATIC_LIBRARY_SUFFIX})
    file(GLOB _a11y_src ${BUI_SOURCE_DIR}/platform/a11y/src/*.rs)
    add_custom_command(OUTPUT ${_a11y_lib}
      COMMAND ${CMAKE_COMMAND} -E env CARGO_TARGET_DIR=${_a11y_dir}
              ${BUI_CARGO} rustc --release --lib --crate-type staticlib
              --manifest-path ${BUI_SOURCE_DIR}/platform/a11y/Cargo.toml
      DEPENDS ${_a11y_src} ${BUI_SOURCE_DIR}/platform/a11y/Cargo.toml
      COMMENT "bui-a11y (cargo)" USES_TERMINAL)
    add_custom_target(bui_a11y_cargo DEPENDS ${_a11y_lib})
    add_dependencies(bui_kit bui_a11y_cargo)
    target_link_libraries(bui_kit PUBLIC ${_a11y_lib})
    # what Rust's standard library needs (cargo rustc -- --print native-static-libs)
    if(WIN32)
      target_link_libraries(bui_kit PUBLIC kernel32 advapi32 ntdll userenv ws2_32 dbghelp)
    else()
      find_package(Threads REQUIRED)
      target_link_libraries(bui_kit PUBLIC Threads::Threads ${CMAKE_DL_LIBS} m)
    endif()
  endif()
else()
  target_sources(bui_kit PRIVATE intern/a11y/a11y_off.c)
endif()

# The desktop shell (intern/shell: tray icons, notifications, taskbar progress, recent files):
# this system's backend, which loads what it uses at run time (shell32, libdbus-1, ...), or the
# API alone (BUI_SHELL off, or a system without one).
set(_bui_shell intern/shell/shell_off.c)
if(BUI_SHELL)
  if(WIN32)
    set(_bui_shell intern/shell/shell.c intern/shell/shell_win32.c)
  elseif(APPLE)
    set(_bui_shell intern/shell/shell.c intern/shell/shell_cocoa.m)
    set_source_files_properties(intern/shell/shell_cocoa.m PROPERTIES COMPILE_OPTIONS "-fno-objc-arc")
  elseif(CMAKE_SYSTEM_NAME MATCHES "Linux|FreeBSD|NetBSD|OpenBSD|DragonFly" AND NOT ANDROID)
    set(_bui_shell intern/shell/shell.c intern/shell/shell_dbus.c)
    target_link_libraries(bui_kit PUBLIC ${CMAKE_DL_LIBS})
  endif()
endif()
target_sources(bui_kit PRIVATE ${_bui_shell})

if(BUI_FILEBROWSER) # Blender's File Browser (editors/space_file)
  target_sources(bui_kit PRIVATE
    intern/kit_filebrowser_paths.c
    intern/kit_filebrowser_posix.c
    intern/kit_filebrowser_win32.c
    intern/kit_filelist.c
    intern/kit_filelist_view.c
    intern/kit_fileops.c
    intern/kit_fsmenu.c)
endif()

# -- image decoders (third party, decoder parts only; see extern/*/README.bui) --
set(_bui_quiet "$<IF:$<C_COMPILER_ID:MSVC>,/w,-w>")
foreach(_f HDR PNM)
  if(BUI_IMAGE_${_f})
    string(TOLOWER ${_f} _lf)
    target_sources(bui_kit PRIVATE intern/image/format_${_lf}.c)
  endif()
endforeach()
# Wuffs: one translation unit (format_wuffs.c includes wuffs-v0.4.c) with the
# modules of the formats this build has
if(BUI_IMAGE_PNG OR BUI_IMAGE_GIF OR BUI_IMAGE_BMP OR BUI_IMAGE_TGA OR BUI_IMAGE_QOI)
  target_sources(bui_kit PRIVATE intern/image/format_wuffs.c)
  target_include_directories(bui_kit PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/extern/wuffs)
  set_source_files_properties(intern/image/format_wuffs.c PROPERTIES COMPILE_OPTIONS "${_bui_quiet}")
endif()
# Video: the OS backend of this platform, loaded at run time; dav1d / FFmpeg
# when asked for (they link those libraries). Its sound plays on the OS's
# audio output (intern/audio), loaded at run time too.
if(BUI_VIDEO)
  if(WIN32)
    target_sources(bui_kit PRIVATE intern/image/movie_mf.c intern/audio/device_wasapi.c)
    target_compile_definitions(bui_kit PRIVATE BUI_VIDEO_MF BUI_AUDIO_WASAPI)
  elseif(APPLE)
    target_sources(bui_kit PRIVATE intern/image/movie_avf.m intern/audio/device_coreaudio.c)
    set_source_files_properties(intern/image/movie_avf.m PROPERTIES COMPILE_OPTIONS "-fno-objc-arc")
    target_compile_definitions(bui_kit PRIVATE BUI_VIDEO_AVF BUI_AUDIO_COREAUDIO)
    # weak: a system without them still starts (one argument each for the linker)
    target_link_libraries(bui_kit PUBLIC "-Wl,-weak_framework,AVFoundation"
                                         "-Wl,-weak_framework,CoreMedia"
                                         "-Wl,-weak_framework,CoreVideo"
                                         "-Wl,-weak_framework,AudioToolbox")
  elseif(CMAKE_SYSTEM_NAME MATCHES "Linux|FreeBSD|NetBSD|OpenBSD|DragonFly")
    # GStreamer and PulseAudio exist here; any other system (Android, Emscripten, ...) builds
    # with no video backend and no audio device: silent, nothing else changes
    target_sources(bui_kit PRIVATE intern/image/movie_gst.c intern/audio/device_pulse.c)
    target_compile_definitions(bui_kit PRIVATE BUI_VIDEO_GST BUI_AUDIO_PULSE)
    find_package(Threads REQUIRED)
    target_link_libraries(bui_kit PUBLIC ${CMAKE_DL_LIBS} Threads::Threads)
  endif()
endif()
if(BUI_IMAGE_JPEG) # libjpeg-turbo 3.1.2: the 8-bit decompressor, no SIMD (it needs NASM)
  file(GLOB _bui_jpeg_src extern/libjpeg-turbo/src/*.c extern/libjpeg-turbo/src/wrapper/*.c)
  list(FILTER _bui_jpeg_src EXCLUDE REGEX "/(jdcolext|jdcol565|jdmrgext|jdmrg565|jstdhuff|jdapistd|jdcoefct|jdcolor|jddctmgr|jddiffct|jdlossls|jdmainct|jdmerge|jdpostct|jdsample|jidctflt|jidctfst|jidctint|jidctred|jutils)\\.c$")
  add_library(bui_jpeg OBJECT ${_bui_jpeg_src}
    extern/libjpeg-turbo/bui/jmem_bui.c extern/libjpeg-turbo/bui/jdecode_8bit_only.c)
  target_include_directories(bui_jpeg PRIVATE extern/libjpeg-turbo/bui extern/libjpeg-turbo/src
    ${BUI_SOURCE_DIR}/core/include)
  target_compile_options(bui_jpeg PRIVATE ${_bui_quiet} $<$<NOT:$<C_COMPILER_ID:MSVC>>:-O2>)
  target_sources(bui_kit PRIVATE intern/image/format_jpeg.c $<TARGET_OBJECTS:bui_jpeg>)
  target_include_directories(bui_kit PRIVATE extern/libjpeg-turbo/bui extern/libjpeg-turbo/src)
endif()
if(BUI_IMAGE_WEBP) # libwebp 1.6.0: decoder + demux
  file(GLOB _bui_webp_src extern/libwebp/src/dec/*.c extern/libwebp/src/dsp/*.c
    extern/libwebp/src/utils/*.c extern/libwebp/src/demux/*.c)
  add_library(bui_webp OBJECT ${_bui_webp_src})
  target_include_directories(bui_webp PRIVATE extern/libwebp)
  # the decode threads call libwebp at once: its DSP init takes a lock with pthreads
  # (upstream's Windows path is lock-free by design)
  if(NOT WIN32)
    target_compile_definitions(bui_webp PRIVATE WEBP_USE_THREAD)
  endif()
  target_compile_options(bui_webp PRIVATE ${_bui_quiet} $<$<NOT:$<C_COMPILER_ID:MSVC>>:-O2>)
  target_sources(bui_kit PRIVATE intern/image/format_webp.c $<TARGET_OBJECTS:bui_webp>)
  target_include_directories(bui_kit PRIVATE extern/libwebp)
endif()
if(BUI_IMAGE_TIFF) # libtiff 4.7.2 (its decoders; JPEG with the libjpeg-turbo above) + zlib 1.4's inflate
  # zlib's symbols are prefixed (z_inflate, ...): a host that links its own zlib keeps it
  add_library(bui_zlib OBJECT extern/zlib/adler32.c extern/zlib/crc32.c extern/zlib/inffast.c
    extern/zlib/inflate.c extern/zlib/inftrees.c extern/zlib/zutil.c)
  target_compile_definitions(bui_zlib PUBLIC Z_PREFIX)
  # Z_PREFIX leaves zlib's one internal global, z_errmsg, as every zlib names it: the copy in a host
  # (D's Phobos, say) would be a second definition
  target_compile_definitions(bui_zlib PRIVATE z_errmsg=bui_z_errmsg)
  target_compile_options(bui_zlib PRIVATE ${_bui_quiet} $<$<NOT:$<C_COMPILER_ID:MSVC>>:-O2>)
  set(_bui_tiff_names aux close codec color compress dir dirinfo dirread dirwrite dumpmode error
    extension fax3 fax3sm flush getimage hash_set luv lzw next open packbits predict print read
    strip swab thunder tile version warning write zip)
  if(BUI_IMAGE_JPEG)
    list(APPEND _bui_tiff_names jpeg)
  endif()
  list(TRANSFORM _bui_tiff_names REPLACE "(.+)" "extern/libtiff/libtiff/tif_\\1.c")
  add_library(bui_tiff OBJECT ${_bui_tiff_names} extern/libtiff/bui/tif_bui.c
    extern/libtiff/bui/tif_decode_only.c)
  target_include_directories(bui_tiff PRIVATE extern/libtiff/bui extern/libtiff/libtiff extern/zlib
    extern/libjpeg-turbo/bui extern/libjpeg-turbo/src ${BUI_SOURCE_DIR}/core/include)
  target_link_libraries(bui_tiff PRIVATE bui_zlib) # Z_PREFIX
  bui_features(bui_tiff) # BUI_IMAGE_JPEG: JPEG_SUPPORT (bui/tiffconf.h)
  if(MINGW)
    # tif_jpeg.c's longjmp out of libjpeg: MinGW's SEH-unwinding longjmp fails there with
    # STATUS_BAD_STACK (GCC's frame address is not the SEH establisher frame); C needs no unwind
    target_compile_definitions(bui_tiff PRIVATE __USE_MINGW_SETJMP_NON_SEH)
  endif()
  target_compile_options(bui_tiff PRIVATE ${_bui_quiet} $<$<NOT:$<C_COMPILER_ID:MSVC>>:-O2>)
  target_sources(bui_kit PRIVATE intern/image/format_tiff.c $<TARGET_OBJECTS:bui_tiff>
    $<TARGET_OBJECTS:bui_zlib>)
  target_include_directories(bui_kit PRIVATE extern/libtiff/bui extern/libtiff/libtiff)
endif()
if(BUI_IMAGE_EXR) # OpenEXRCore 3.5.5 (C) with the libdeflate 1.26 it ships; no zstd, no HTJ2K
  file(GLOB _bui_exr_src extern/openexr/src/lib/OpenEXRCore/*.c)
  add_library(bui_exr OBJECT ${_bui_exr_src} extern/openexr/bui/exr_bui.c)
  target_include_directories(bui_exr PRIVATE extern/openexr/bui extern/openexr/src/lib/OpenEXRCore)
  target_compile_options(bui_exr PRIVATE ${_bui_quiet} $<$<NOT:$<C_COMPILER_ID:MSVC>>:-O2>)
  target_sources(bui_kit PRIVATE intern/image/format_exr.c $<TARGET_OBJECTS:bui_exr>)
  target_include_directories(bui_kit PRIVATE extern/openexr/bui extern/openexr/src/lib/OpenEXRCore)
endif()
if(BUI_IMAGE_AVIF) # libavif 1.4.2 (decoder, its bundled libyuv subset) + dav1d 1.5.5 (C, no asm)
  set(_bui_dav1d_inc extern/dav1d/bui extern/dav1d extern/dav1d/include extern/dav1d/include/dav1d)
  if(MSVC)
    list(APPEND _bui_dav1d_inc extern/dav1d/include/compat/msvc) # stdatomic.h
  endif()
  file(GLOB _bui_dav1d_src extern/dav1d/src/*.c)
  set(_bui_dav1d_tmpl ${_bui_dav1d_src})
  list(FILTER _bui_dav1d_src EXCLUDE REGEX "_tmpl\\.c$")
  list(FILTER _bui_dav1d_tmpl INCLUDE REGEX "_tmpl\\.c$")
  if(WIN32)
    list(APPEND _bui_dav1d_src extern/dav1d/src/win32/thread.c) # pthreads on Win32 threads
  endif()
  # dav1d's *_tmpl.c are compiled once per pixel depth (meson: libdav1d_bitdepth_objs)
  add_library(bui_dav1d OBJECT ${_bui_dav1d_src})
  add_library(bui_dav1d_8 OBJECT ${_bui_dav1d_tmpl})
  add_library(bui_dav1d_16 OBJECT ${_bui_dav1d_tmpl})
  target_compile_definitions(bui_dav1d_8 PRIVATE BITDEPTH=8)
  target_compile_definitions(bui_dav1d_16 PRIVATE BITDEPTH=16)
  file(GLOB _bui_avif_src extern/libavif/src/*.c extern/libavif/third_party/libyuv/source/*.c)
  add_library(bui_avif OBJECT ${_bui_avif_src})
  target_compile_definitions(bui_avif PRIVATE AVIF_CODEC_DAV1D=1)
  target_include_directories(bui_avif PRIVATE extern/libavif/include
    extern/libavif/third_party/libyuv/include extern/dav1d/include)
  foreach(_t bui_dav1d bui_dav1d_8 bui_dav1d_16 bui_avif)
    if(NOT _t STREQUAL "bui_avif")
      target_include_directories(${_t} PRIVATE ${_bui_dav1d_inc})
    endif()
    set_target_properties(${_t} PROPERTIES C_STANDARD 11)
    target_compile_options(${_t} PRIVATE ${_bui_quiet} $<$<NOT:$<C_COMPILER_ID:MSVC>>:-O2>)
    target_compile_definitions(${_t} PRIVATE $<$<C_COMPILER_ID:MSVC>:_CRT_SECURE_NO_WARNINGS>)
    target_sources(bui_kit PRIVATE $<TARGET_OBJECTS:${_t}>)
  endforeach()
  target_sources(bui_kit PRIVATE intern/image/format_avif.c)
  target_include_directories(bui_kit PRIVATE extern/libavif/include)
endif()
if(BUI_IMAGE_JXL) # libjxl 0.13.0's decoder (jxl_dec: no boxes, no JPEG reconstruction) + Highway 1.4.0
  file(GLOB_RECURSE _bui_jxl_src extern/libjxl/lib/jxl/*.cc)
  add_library(bui_jxl OBJECT ${_bui_jxl_src} extern/highway/hwy/abort.cc
    extern/highway/hwy/aligned_allocator.cc extern/highway/hwy/per_target.cc
    extern/highway/hwy/print.cc extern/highway/hwy/targets.cc extern/highway/hwy/timer.cc)
  target_include_directories(bui_jxl PRIVATE extern/libjxl/bui extern/libjxl
    extern/libjxl/lib/include extern/highway)
  # libjxl's CMake: its static build's defines, and the SIMD targets it builds by default
  # (Highway dispatches at run time to the best the CPU has)
  target_compile_definitions(bui_jxl PRIVATE JXL_STATIC_DEFINE JXL_INTERNAL_LIBRARY_BUILD
    HWY_STATIC_DEFINE JPEGXL_ENABLE_TRANSCODE_JPEG=0 JPEGXL_ENABLE_BOXES=0
    "HWY_DISABLED_TARGETS=(HWY_AVX3|HWY_AVX3_SPR|HWY_AVX3_ZEN4|HWY_RVV|HWY_SSSE3|HWY_SVE_256|HWY_SVE|HWY_SVE2|HWY_SVE2_128)"
    $<$<CXX_COMPILER_ID:MSVC>:_CRT_SECURE_NO_WARNINGS>)
  set_target_properties(bui_jxl PROPERTIES CXX_STANDARD 17 CXX_VISIBILITY_PRESET hidden
    VISIBILITY_INLINES_HIDDEN ON)
  target_compile_options(bui_jxl PRIVATE ${_bui_quiet}
    $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-O2 -fno-rtti -fno-exceptions>)
  target_sources(bui_kit PRIVATE intern/image/format_jxl.c $<TARGET_OBJECTS:bui_jxl>)
  target_include_directories(bui_kit PRIVATE extern/libjxl/bui extern/libjxl/lib/include)
  target_compile_definitions(bui_kit PRIVATE JXL_STATIC_DEFINE)
endif()
# HEIC: the OS decoder, loaded at run time like video above (nothing vendored: HEVC patents)
if(BUI_IMAGE_HEIC)
  target_sources(bui_kit PRIVATE intern/image/format_heic.c)
  if(WIN32)
    target_sources(bui_kit PRIVATE intern/image/heic_wic.c)
    target_compile_definitions(bui_kit PRIVATE BUI_HEIC_WIC)
  elseif(APPLE)
    target_sources(bui_kit PRIVATE intern/image/heic_imageio.c)
    target_compile_definitions(bui_kit PRIVATE BUI_HEIC_IMAGEIO)
    target_link_libraries(bui_kit PUBLIC "-Wl,-weak_framework,ImageIO" "-framework CoreFoundation")
  elseif(CMAKE_SYSTEM_NAME MATCHES "Linux|FreeBSD|NetBSD|OpenBSD|DragonFly")
    target_sources(bui_kit PRIVATE intern/image/heic_libheif.c)
    target_compile_definitions(bui_kit PRIVATE BUI_HEIC_LIBHEIF)
    find_package(Threads REQUIRED)
    target_link_libraries(bui_kit PUBLIC ${CMAKE_DL_LIBS} Threads::Threads)
  else()
    target_compile_definitions(bui_kit PRIVATE BUI_HEIC_NO_BACKEND) # HEIC reads as missing
  endif()
endif()
# worker threads: bui_core links the platform's threads (Threads::Threads)
target_include_directories(bui_kit PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/include)
target_include_directories(bui_kit PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/intern)
# bui_core: Blender's color math and UTF-8 cursor stepping; bui_gpu: raster.
target_link_libraries(bui_kit PUBLIC bui::gpu bui::core)
if(NOT MSVC)
  target_compile_options(bui_kit PRIVATE -O2)
  target_link_libraries(bui_kit PRIVATE m)
endif()
if(WIN32)
  target_link_libraries(bui_kit PRIVATE user32 gdi32 imm32)
elseif(APPLE)
  # Native windows on macOS: an AppKit host after Blender's GHOST Cocoa backend.
  # Objective-C (enable_language(OBJC) is in the top-level CMakeLists.txt) with
  # manual retain/release, as GHOST; kit_window_null.c compiles to nothing.
  target_sources(bui_kit PRIVATE intern/kit_window_cocoa.m)
  set_source_files_properties(intern/kit_window_cocoa.m PROPERTIES COMPILE_OPTIONS "-fno-objc-arc")
  target_compile_definitions(bui_kit PRIVATE BUI_WINDOW_COCOA)
  target_link_libraries(bui_kit PUBLIC "-framework Cocoa" "-framework CoreGraphics" objc)
elseif(ANDROID)
  # Native window on Android: the activity's surface (android_native_app_glue), software raster.
  target_sources(bui_kit PRIVATE intern/kit_window_android.c)
  target_compile_definitions(bui_kit PRIVATE BUI_WINDOW_ANDROID)
  target_include_directories(bui_kit PRIVATE ${CMAKE_ANDROID_NDK}/sources/android/native_app_glue)
  target_link_libraries(bui_kit PUBLIC android log)
elseif(UNIX AND NOT APPLE)
  # Native windows on X11 (Wayland sessions run it through XWayland).
  # Only the headers are needed: libX11 is dlopen()ed at run time, so builds
  # (and wheels) run without X11 installed and open windows where it is.
  find_path(BUI_X11_INCLUDE X11/Xlib.h)
  if(BUI_X11_INCLUDE)
    target_compile_definitions(bui_kit PRIVATE BUI_WINDOW_X11)
    target_include_directories(bui_kit PRIVATE ${BUI_X11_INCLUDE})
    target_link_libraries(bui_kit PUBLIC ${CMAKE_DL_LIBS})
  else()
    message(STATUS "BUI: X11 headers not found; no native Linux windows (Tk / headless only)")
  endif()

  # Native Wayland windows, as Blender's GHOST_SystemWayland (xdg-shell, wl_shm, wl_seat with
  # xkbcommon, wl_data_device). Like X11, nothing is linked: Blender's wayland_dynload loads
  # libwayland-client / libwayland-cursor and the host dlopen()s libxkbcommon when the first
  # window opens. Building needs the headers, wayland-scanner and wayland-protocols; without
  # them (or with -DBUI_WAYLAND=OFF) windows are X11 only, as before. A session with
  # WAYLAND_DISPLAY gets Wayland windows (CPU renderer), unless the GPU renderer is on: that is
  # OpenGL through GLX, so it keeps X11 (XWayland) until Wayland has its own EGL renderer.
  option(BUI_WAYLAND "Native Wayland windows (libwayland is loaded at run time)" ON)
  if(BUI_WAYLAND AND BUI_X11_INCLUDE)
    find_package(PkgConfig QUIET)
    if(PkgConfig_FOUND)
      pkg_check_modules(BUI_WL QUIET wayland-client wayland-cursor xkbcommon wayland-protocols)
      pkg_get_variable(BUI_WL_PROTOCOLS wayland-protocols pkgdatadir)
      find_program(BUI_WL_SCANNER wayland-scanner)
    endif()
    if(BUI_WL_FOUND AND BUI_WL_PROTOCOLS AND BUI_WL_SCANNER)
      set(_wl_gen ${CMAKE_CURRENT_BINARY_DIR}/wayland)
      file(MAKE_DIRECTORY ${_wl_gen})
      set(_wl_src)
      # wlr-layer-shell is not in wayland-protocols: kit/extern/wayland_protocols has it
      set(_wl_xml ${BUI_WL_PROTOCOLS}/stable/xdg-shell/xdg-shell.xml
                  ${BUI_WL_PROTOCOLS}/unstable/xdg-decoration/xdg-decoration-unstable-v1.xml
                  ${CMAKE_CURRENT_SOURCE_DIR}/extern/wayland_protocols/wlr-layer-shell-unstable-v1.xml)
      # cursor-shape (wayland-protocols 1.32+), as GHOST: the compositor draws the cursor, so no
      # cursor theme is loaded. Its code refers to the tablet tool interface.
      set(_wl_cursor_shape ${BUI_WL_PROTOCOLS}/staging/cursor-shape/cursor-shape-v1.xml)
      set(_wl_tablet ${BUI_WL_PROTOCOLS}/stable/tablet/tablet-v2.xml)
      if(NOT EXISTS ${_wl_tablet})
        set(_wl_tablet ${BUI_WL_PROTOCOLS}/unstable/tablet/tablet-unstable-v2.xml)
      endif()
      if(EXISTS ${_wl_cursor_shape} AND EXISTS ${_wl_tablet})
        list(APPEND _wl_xml ${_wl_cursor_shape} ${_wl_tablet})
        target_compile_definitions(bui_kit PRIVATE BUI_WL_CURSOR_SHAPE)
      endif()
      # pointer-gestures (wayland-protocols 1.4+): touchpad pinches, as GHOST
      set(_wl_gestures
          ${BUI_WL_PROTOCOLS}/unstable/pointer-gestures/pointer-gestures-unstable-v1.xml)
      if(EXISTS ${_wl_gestures})
        list(APPEND _wl_xml ${_wl_gestures})
        target_compile_definitions(bui_kit PRIVATE BUI_WL_POINTER_GESTURES)
      endif()
      foreach(_x ${_wl_xml})
        get_filename_component(_n ${_x} NAME_WE)
        add_custom_command(
          OUTPUT ${_wl_gen}/${_n}-client-protocol.h ${_wl_gen}/${_n}-client-protocol.c
          COMMAND ${BUI_WL_SCANNER} client-header ${_x} ${_wl_gen}/${_n}-client-protocol.h
          COMMAND ${BUI_WL_SCANNER} private-code ${_x} ${_wl_gen}/${_n}-client-protocol.c
          DEPENDS ${_x})
        list(APPEND _wl_src ${_wl_gen}/${_n}-client-protocol.c)
      endforeach()
      target_sources(bui_kit PRIVATE intern/kit_window_wayland.c intern/kit_window_linux.c
        extern/wayland_dynload/wayland_dynload_client.c
        extern/wayland_dynload/wayland_dynload_cursor.c
        extern/wayland_dynload/wayland_dynload_utils.c ${_wl_src})
      target_compile_definitions(bui_kit PRIVATE BUI_WINDOW_WAYLAND)
      target_include_directories(bui_kit PRIVATE ${_wl_gen} extern/wayland_dynload
        ${BUI_WL_INCLUDE_DIRS})
    else()
      message(STATUS "BUI: Wayland headers, wayland-scanner or wayland-protocols not found; "
                     "Linux windows are X11 (Wayland sessions through XWayland)")
    endif()
  endif()
endif()
