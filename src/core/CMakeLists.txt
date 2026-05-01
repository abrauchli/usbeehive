add_library(whatcablecore STATIC
    SysfsReader.cpp
    UsbDevice.cpp
    TypeCPort.cpp
    PowerDelivery.cpp
    PDDecoder.cpp
    CableInfo.cpp
    VendorDB.cpp
    UsbClassDB.cpp
    DeviceSummary.cpp
    ChargingDiagnostic.cpp
    UDevMonitor.cpp
    DeviceManager.cpp
)

set_target_properties(whatcablecore PROPERTIES POSITION_INDEPENDENT_CODE ON)
target_include_directories(whatcablecore PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
target_link_libraries(whatcablecore PUBLIC ${UDEV_LIBRARIES})
target_include_directories(whatcablecore PRIVATE ${UDEV_INCLUDE_DIRS})
