namespace demo { struct Box { int operator[](int); }; }
demo::Box &receiver();
int tick();
int f(int x) {
    if (x) return receiver().demo::Box::operator[](tick());
    return 0;
}
