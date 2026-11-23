using namespace std;

struct item {};
template <class T> struct box { T value; };

int unknown_qualified(int n) {
    if (n) {
        std::vector<std::string> lines;
        while (n > 0)
            n--;
    }
    return n;
}

int unknown_unqualified(int n) {
    if (n) {
        vector<string> names;
        while (n > 0)
            n--;
    }
    return n;
}

int unknown_initialized(const std::vector<std::string>& source, int n) {
    if (n) {
        std::vector<std::string> copy = source;
        while (n > 0)
            n--;
    }
    return n;
}

int known_template(int n) {
    if (n) {
        box<item> held;
        while (n > 0)
            n--;
    }
    return n;
}

int keyword_argument(int n) {
    if (n) {
        std::vector<int> counts;
        while (n > 0)
            n--;
    }
    return n;
}

int pointer_argument(int n) {
    if (n) {
        std::vector<std::string*> pointers;
        while (n > 0)
            n--;
    }
    return n;
}

int two_arguments(int n) {
    if (n) {
        std::map<std::string, int> table;
        while (n > 0)
            n--;
    }
    return n;
}
