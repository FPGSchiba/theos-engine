@echo off
rem Compile GLSL shaders to SPIR-V binary format
C:/VulkanSDK/1.4.321.1/Bin/glslc.exe triangle.vert -o triangle.vert.spv
C:/VulkanSDK/1.4.321.1/Bin/glslc.exe triangle.frag -o triangle.frag.spv
pause