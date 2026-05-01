#include "greet.h"
#include <iostream>

int main() {
    std::string msg = greet("World");
    std::cout << msg << std::endl;
    return 0;
}
