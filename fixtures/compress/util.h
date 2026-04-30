#pragma once

// Calculate the maximum of two values
template <typename T>
T max(T a, T b) {
    /* This is a multi-line
       comment that should
       be removed during compression */
    return a > b ? a : b;
}

// Calculate the minimum of two values
template <typename T>
T min(T a, T b) {
    return a < b ? a : b;  // inline comment
}
