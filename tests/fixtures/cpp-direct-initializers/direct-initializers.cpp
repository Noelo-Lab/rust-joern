struct Item;
struct Box {
  explicit Box(Item* item);
  explicit Box(int value);
  Box(Item* first, Item* second);
  Item* item;
};

Item* make();
Item* spare;

void variable(Item* p) { Box box(p); }
void conditional(Item* p, bool c) { Box box(c ? p : spare); }
void compared(Item* p) { Box box((p == spare) ? p : nullptr); }
void called() { Box box(make()); }
void mixed(Item* p) { Box box(p, make()); }
void literal() { Box box(7); }
void negated(int n) { Box box(-n); }
void sum(int n) { Box box(n + 1); }
void member(Box* other) { Box box(other->item); }
void null_pointer() { Box box(nullptr); }

struct Holder {
  Item* item;
  void own();
};
void Holder::own() { Box box(this->item); }

// A parameter declaration keeps a local function prototype.
void prototype() { Box made(Item* p); }
