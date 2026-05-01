// Derived from WhatCable by Darryl Morley (https://github.com/darrylmorley/whatcable)
// Linux sysfs equivalent of the IOKit property reading in the original.
#pragma once

#include <map>
#include <optional>
#include <string>
#include <vector>
#include <cstdint>

namespace WhatCable {

class SysfsReader {
public:
    static std::string readAttribute(const std::string &path);
    static std::optional<int> readIntAttribute(const std::string &path);
    static std::optional<uint32_t> readHexAttribute(const std::string &path);
    static std::vector<std::string> listSubdirectories(const std::string &path);
    static std::map<std::string, std::string> readAllAttributes(const std::string &dirPath);
    static bool pathExists(const std::string &path);
};

} // namespace WhatCable
