namespace n { class A { public: int f(int x) { if (int y=x) return y; for (auto z : xs) x += z; try { throw x; } catch (...) { return 0; } return x; } }; template<class T> T id(T x) { return x; } }
