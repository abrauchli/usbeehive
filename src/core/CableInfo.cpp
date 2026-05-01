// Derived from WhatCable by Darryl Morley (https://github.com/darrylmorley/whatcable)
#include "CableInfo.h"
#include "VendorDB.h"

namespace WhatCable {

std::optional<CableInfo> CableInfo::fromTypeCCable(const TypeCCable &cable)
{
    CableInfo info;
    info.cableType = cable.type;
    info.plugType = cable.plugType;
    info.isActive = (cable.type == "active");
    info.isPassive = (cable.type == "passive");
    info.rawAttributes = cable.rawAttributes;

    if (cable.identity) {
        const auto &id = *cable.identity;
        info.vendorId = id.vendorId;
        info.vendorName = VendorDB::lookup(id.vendorId);

        if (!id.vdos.empty()) {
            auto hdr = decodeIDHeader(id.vdos[0]);
            bool active = (hdr.ufpProductType == ProductType::ActiveCable);

            if (id.vdos.size() > 3) {
                auto cableVdo = decodeCableVDO(id.vdos[3], active);
                info.speed = cableVdo.speed;
                info.currentRating = cableVdo.currentRating;
                info.maxWatts = cableVdo.maxWatts;
                info.isActive = cableVdo.isActive;
                info.isPassive = !cableVdo.isActive;
            }
        }
    }

    return info;
}

} // namespace WhatCable
