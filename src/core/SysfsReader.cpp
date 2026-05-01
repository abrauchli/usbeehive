// Derived from WhatCable by Darryl Morley (https://github.com/darrylmorley/whatcable)
#include "SysfsReader.h"
#include <algorithm>
#include <cctype>
#include <cstdlib>
#include <filesystem>
#include <fstream>

namespace fs = std::filesystem;

namespace WhatCable {

namespace {

std::string trim(std::string s)
{
    auto not_space_front = [](unsigned char c) { return !std::isspace(c); };
    auto not_space_back = [](unsigned char c) { return !std::isspace(c); };
    s.erase(s.begin(), std::find_if(s.begin(), s.end(), not_space_front));
    s.erase(std::find_if(s.rbegin(), s.rend(), not_space_back).base(), s.end());
    return s;
}

} // namespace

std::string SysfsReader::readAttribute(const std::string &path)
{
    std::ifstream file(path);
    if (!file)
        return {};
    std::string line;
    std::getline(file, line);
    return trim(std::move(line));
}

std::optional<int> SysfsReader::readIntAttribute(const std::string &path)
{
    const std::string val = readAttribute(path);
    if (val.empty())
        return std::nullopt;
    char *end = nullptr;
    long result = std::strtol(val.c_str(), &end, 10);
    if (end == val.c_str())
        return std::nullopt;
    return static_cast<int>(result);
}

std::optional<uint32_t> SysfsReader::readHexAttribute(const std::string &path)
{
    std::string val = readAttribute(path);
    if (val.empty())
        return std::nullopt;
    if (val.size() >= 2 && val[0] == '0' && (val[1] == 'x' || val[1] == 'X'))
        val.erase(0, 2);
    char *end = nullptr;
    unsigned long result = std::strtoul(val.c_str(), &end, 16);
    if (end == val.c_str())
        return std::nullopt;
    return static_cast<uint32_t>(result);
}

std::vector<std::string> SysfsReader::listSubdirectories(const std::string &path)
{
    std::vector<std::string> out;
    std::error_code ec;
    const fs::path base(path);
    if (!fs::is_directory(base, ec))
        return out;
    for (const auto &e : fs::directory_iterator(base, fs::directory_options::skip_permission_denied, ec)) {
        if (!ec && e.is_directory(ec))
            out.push_back(e.path().filename().string());
    }
    std::sort(out.begin(), out.end());
    return out;
}

std::map<std::string, std::string> SysfsReader::readAllAttributes(const std::string &dirPath)
{
    std::map<std::string, std::string> attrs;
    std::error_code ec;
    const fs::path dir(dirPath);
    if (!fs::is_directory(dir, ec))
        return attrs;
    for (const auto &e : fs::directory_iterator(dir, fs::directory_options::skip_permission_denied, ec)) {
        if (!ec && e.is_regular_file(ec)) {
            const std::string fname = e.path().filename().string();
            const std::string val = readAttribute(e.path().string());
            if (!val.empty())
                attrs[fname] = val;
        }
    }
    return attrs;
}

bool SysfsReader::pathExists(const std::string &path)
{
    std::error_code ec;
    return fs::exists(fs::path(path), ec);
}

} // namespace WhatCable
