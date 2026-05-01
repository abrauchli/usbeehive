// Derived from WhatCable by Darryl Morley (https://github.com/darrylmorley/whatcable)
// Port of PDVDO.swift — USB PD 3.x VDO bit-field decoding
#include "PDDecoder.h"

namespace WhatCable {

std::string productTypeLabel(ProductType type)
{
    switch (type) {
    case ProductType::Hub:          return "USB Hub";
    case ProductType::Peripheral:   return "USB Peripheral";
    case ProductType::PassiveCable: return "Passive Cable";
    case ProductType::ActiveCable:  return "Active Cable";
    case ProductType::AMA:          return "Alternate Mode Adapter";
    case ProductType::VPD:          return "VCONN-Powered Device";
    case ProductType::Other:        return "Other";
    default:                        return "Unknown";
    }
}

std::string cableSpeedLabel(CableSpeed speed)
{
    switch (speed) {
    case CableSpeed::USB20:     return "USB 2.0";
    case CableSpeed::USB32Gen1: return "USB 3.2 Gen 1 (5 Gbps)";
    case CableSpeed::USB32Gen2: return "USB 3.2 Gen 2 (10 Gbps)";
    case CableSpeed::USB4Gen3:  return "USB4 Gen 3 (20/40 Gbps)";
    case CableSpeed::USB4Gen4:  return "USB4 Gen 4 (40/80 Gbps)";
    default:                    return "Unknown";
    }
}

int cableSpeedMaxGbps(CableSpeed speed)
{
    switch (speed) {
    case CableSpeed::USB20:     return 0;
    case CableSpeed::USB32Gen1: return 5;
    case CableSpeed::USB32Gen2: return 10;
    case CableSpeed::USB4Gen3:  return 40;
    case CableSpeed::USB4Gen4:  return 80;
    default:                    return 0;
    }
}

std::string cableCurrentLabel(CableCurrent current)
{
    switch (current) {
    case CableCurrent::USBDefault: return "USB Default";
    case CableCurrent::ThreeAmp:   return "3A";
    case CableCurrent::FiveAmp:    return "5A";
    default:                       return "Unknown";
    }
}

double cableCurrentMaxAmps(CableCurrent current)
{
    switch (current) {
    case CableCurrent::ThreeAmp: return 3.0;
    case CableCurrent::FiveAmp:  return 5.0;
    default:                     return 0.9;
    }
}

IDHeaderVDO decodeIDHeader(uint32_t vdo)
{
    IDHeaderVDO hdr;
    hdr.usbCommCapableAsHost = (vdo >> 31) & 1;
    hdr.usbCommCapableAsDevice = (vdo >> 30) & 1;
    hdr.modalOperation = (vdo >> 26) & 1;
    hdr.vendorId = static_cast<uint16_t>(vdo & 0xFFFF);

    uint8_t ufpBits = (vdo >> 27) & 0x7;
    switch (ufpBits) {
    case 1: hdr.ufpProductType = ProductType::Hub; break;
    case 2: hdr.ufpProductType = ProductType::Peripheral; break;
    case 3: hdr.ufpProductType = ProductType::PassiveCable; break;
    case 4: hdr.ufpProductType = ProductType::ActiveCable; break;
    case 5: hdr.ufpProductType = ProductType::AMA; break;
    case 6: hdr.ufpProductType = ProductType::VPD; break;
    default: hdr.ufpProductType = ProductType::Undefined; break;
    }

    uint8_t dfpBits = (vdo >> 23) & 0x7;
    switch (dfpBits) {
    case 1: hdr.dfpProductType = ProductType::Hub; break;
    case 2: hdr.dfpProductType = ProductType::Peripheral; break;
    default: hdr.dfpProductType = ProductType::Undefined; break;
    }

    return hdr;
}

CableVDO decodeCableVDO(uint32_t vdo, bool isActive)
{
    CableVDO cable;
    cable.isActive = isActive;

    uint8_t speedBits = vdo & 0x7;
    switch (speedBits) {
    case 0: cable.speed = CableSpeed::USB20; break;
    case 1: cable.speed = CableSpeed::USB32Gen1; break;
    case 2: cable.speed = CableSpeed::USB32Gen2; break;
    case 3: cable.speed = CableSpeed::USB4Gen3; break;
    case 4: cable.speed = CableSpeed::USB4Gen4; break;
    default: cable.speed = CableSpeed::USB20; break;
    }

    cable.vbusThroughCable = (vdo >> 4) & 1;

    uint8_t currentBits = (vdo >> 5) & 0x3;
    switch (currentBits) {
    case 0: cable.currentRating = CableCurrent::USBDefault; break;
    case 1: cable.currentRating = CableCurrent::ThreeAmp; break;
    case 2: cable.currentRating = CableCurrent::FiveAmp; break;
    default: cable.currentRating = CableCurrent::USBDefault; break;
    }

    uint8_t voltageBits = (vdo >> 9) & 0x3;
    switch (voltageBits) {
    case 0: cable.maxVbusVolts = 20; break;
    case 1: cable.maxVbusVolts = 30; break;
    case 2: cable.maxVbusVolts = 40; break;
    case 3: cable.maxVbusVolts = 50; break;
    }

    double amps = cableCurrentMaxAmps(cable.currentRating);
    cable.maxWatts = static_cast<int>(cable.maxVbusVolts * amps);

    return cable;
}

} // namespace WhatCable
