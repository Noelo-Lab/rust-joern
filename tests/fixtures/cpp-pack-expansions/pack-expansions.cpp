int f(int);
int h(int, int, int);

template <std::size_t... seq>
std::array<short, sizeof...(seq)> table() {
  return std::array<short, sizeof...(seq)>{static_cast<short>(f(seq))...};
}

template <std::size_t... indexes>
std::array<char, sizeof...(indexes)> mask(int offset, int size) {
  return {static_cast<char>((offset + indexes) % size)...};
}

template <class... T>
int forward(int n, T... values) {
  int all[] = {f(values)...};
  while (n > 0)
    n = h(values...);
  return n + all[0];
}
