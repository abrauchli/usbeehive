#pragma once

#include <string>
#include <cstdint>

namespace WhatCable {

class UsbClassDB {
public:
    static std::string className(uint8_t classCode);
    static std::string interfaceClassName(uint8_t classCode, uint8_t subClass = 0);
};

} // namespace WhatCable
