template <class T> struct Box {
  typedef T *cursor;
  struct Slot {
    T value;
  };
};

struct Tray {
  Box<int>::cursor first(int limit);
  Box<char>::Slot pick(Box<char>::Slot slot, int index);
  int walk(Box<int>::cursor at, int count);
  void touch(Box<int>::cursor at);
};

Box<int>::cursor Tray::first(int limit) {
  if (limit > 0) {
    return 0;
  }
  return 0;
}

Box<char>::Slot Tray::pick(Box<char>::Slot slot, int index) {
  if (index > 0) {
    slot.value = 'a';
  }
  return slot;
}

int Tray::walk(Box<int>::cursor at, int count) {
  int total = 0;
  while (count > 0) {
    if (*at > 0) {
      touch(at);
    } else {
      total += *at;
    }
    count--;
  }
  return total;
}

Box<int>::cursor last(int limit);
