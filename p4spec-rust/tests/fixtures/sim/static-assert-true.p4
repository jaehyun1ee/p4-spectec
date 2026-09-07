#include <core.p4>

const bool check_default = static_assert(true);
const bool check_custom = static_assert(true, "unused");
