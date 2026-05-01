// Derived from WhatCable by Darryl Morley (https://github.com/darrylmorley/whatcable)
// Port of VendorDB.swift — USB VID to vendor name lookup
#pragma once

#include <string>
#include <cstdint>

namespace WhatCable {

class VendorDB {
public:
    static std::string lookup(uint16_t vendorId);
};

} // namespace WhatCable
