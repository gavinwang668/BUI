# bui_gpu tests
# SPDX-License-Identifier: GPL-2.0-or-later

add_executable(bui_gpu_parity test_gpu_parity.c)
target_link_libraries(bui_gpu_parity PRIVATE bui_gpu)
if(NOT MSVC)
  target_link_libraries(bui_gpu_parity PRIVATE m)
endif()
add_test(NAME gpu_parity COMMAND bui_gpu_parity)

add_executable(bui_raster_alpha test_raster_alpha.c)
target_link_libraries(bui_raster_alpha PRIVATE bui_gpu)
add_test(NAME raster_alpha COMMAND bui_raster_alpha)

add_executable(bui_gpu_effect test_gpu_effect.c)
target_link_libraries(bui_gpu_effect PRIVATE bui_gpu)
if(NOT MSVC)
  target_link_libraries(bui_gpu_effect PRIVATE m)
endif()
add_test(NAME gpu_effect COMMAND bui_gpu_effect)

add_executable(bui_gpu_choice test_gpu_choice.c)
target_link_libraries(bui_gpu_choice PRIVATE bui_gpu)
add_test(NAME gpu_choice COMMAND bui_gpu_choice)

# Windows: the same GPU tests on Direct3D 12 (Direct3D 11 is the default). test_gpu_api.h fails a
# run whose device fell back to Direct3D 11.
if(WIN32)
  foreach(t gpu_parity raster_alpha gpu_effect)
    add_test(NAME ${t}_d3d12 COMMAND bui_${t})
    set_tests_properties(${t}_d3d12 PROPERTIES ENVIRONMENT "BUI_GPU_API=d3d12")
  endforeach()
endif()
