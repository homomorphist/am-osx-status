#include <std/io.pat>

struct DateInspector {
    u32 value;
} [[format_read("fmt")]];

fn fmt(DateInspector inspector) {
    std::time::EpochTime time = std::time::to_local(inspector.value - 2082819600)
    return std::time::format(time)
};

DateInspector inspector @ $ [[name("MacOS DateTime")]];
