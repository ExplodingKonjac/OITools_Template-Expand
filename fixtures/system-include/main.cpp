#include "answer.h"
#include <vector>
#include <algorithm>

int main() {
    std::vector<int> v;
    v.push_back(answer());
    std::sort(v.begin(), v.end());
    return v[0];
}
