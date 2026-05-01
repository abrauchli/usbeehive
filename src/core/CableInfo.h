// Derived from WhatCable by Darryl Morley (https://github.com/darrylmorley/whatcable)
// Reads cable e-marker data from /sys/class/typec/portN-cable/identity/
#pragma once

#include "TypeCPort.h"
#include "PDDecoder.h"
#include <map>
#include <optional>
#include <string>

namespace WhatCable {

struct CableInfo {
    bool isActive = false;
    bool isPassive = false;
    std::string cableType;           // "active", "passive"
    std::string plugType;

    std::optional<CableSpeed> speed;
    std::optional<CableCurrent> currentRating;
    int maxWatts = 0;

    uint16_t vendorId = 0;
    std::string vendorName;

    std::map<std::string, std::string> rawAttributes;

    static std::optional<CableInfo> fromTypeCCable(const TypeCCable &cable);
};

} // namespace WhatCable
