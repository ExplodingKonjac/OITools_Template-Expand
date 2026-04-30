// Template expand test with compression
// This file has lots of comments and whitespace

#include "util.h"
#include <cstdio>

int main() {
    int x = 42;
    int y = 13;

    /* Print the maximum */
    int m = max(x, y);
    printf("max=%d\n", m);

    // Return the minimum
    int n = min(x, y);
    printf("min=%d\n", n);

    return 0;
}
