// Derived from WhatCable by Darryl Morley (https://github.com/darrylmorley/whatcable)
// Port of PDVDO.swift — USB Power Delivery VDO bit-field decoding
#pragma once

#include <string>
#include <cstdint>

namespace WhatCable {

enum class ProductType : uint8_t {
    Undefined = 0,
    Hub,
    Peripheral,
    PassiveCable,
    ActiveCable,
    AMA,         // Alternate Mode Adapter
    VPD,         // VCONN-Powered Device
    Other
};

enum class CableSpeed : uint8_t {
    USB20 = 0,
    USB32Gen1,   // 5 Gbps
    USB32Gen2,   // 10 Gbps
    USB4Gen3,    // 20 Gbps (single-lane) / 40 Gbps (dual-lane)
    USB4Gen4     // 40 Gbps (single-lane) / 80 Gbps (dual-lane)
};

enum class CableCurrent : uint8_t {
    USBDefault = 0,
    ThreeAmp,
    FiveAmp
};

struct IDHeaderVDO {
    bool usbCommCapableAsHost = false;
    bool usbCommCapableAsDevice = false;
    bool modalOperation = false;
    ProductType ufpProductType = ProductType::Undefined;
    ProductType dfpProductType = ProductType::Undefined;
    uint16_t vendorId = 0;
};

struct CableVDO {
    CableSpeed speed = CableSpeed::USB20;
    CableCurrent currentRating = CableCurrent::USBDefault;
    bool vbusThroughCable = false;
    int maxVbusVolts = 20;
    bool isActive = false;
    int maxWatts = 0;
};

std::string productTypeLabel(ProductType type);
std::string cableSpeedLabel(CableSpeed speed);
int cableSpeedMaxGbps(CableSpeed speed);
std::string cableCurrentLabel(CableCurrent current);
double cableCurrentMaxAmps(CableCurrent current);

IDHeaderVDO decodeIDHeader(uint32_t vdo);
CableVDO decodeCableVDO(uint32_t vdo, bool isActive);

} // namespace WhatCable
