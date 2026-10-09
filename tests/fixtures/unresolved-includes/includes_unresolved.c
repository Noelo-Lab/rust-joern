#include <rust_joern_missing_system_header.h>
# include "rust_joern_missing_local_header.h"
#include <stdio.h>
int conditional(int x) { if (x > 0) return 1; return 0; }
int loop(int x) {
    #include "rust_joern_missing_body_header.h"
    while (x > 0) --x;
    return x;
}
