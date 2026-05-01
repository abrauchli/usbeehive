add_executable(whatcable-linux main.cpp)
target_link_libraries(whatcable-linux PRIVATE whatcablecore)
target_compile_definitions(whatcable-linux PRIVATE "WHATCABLE_LINUX_VERSION=\"${PROJECT_VERSION}\"")
install(TARGETS whatcable-linux DESTINATION bin)
