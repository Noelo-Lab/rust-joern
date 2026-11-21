namespace demo {
int tick(int);
struct Rule { std::string name() const; };
}

void report(const std::string& path, const demo::Rule& rule, int count) {
  std::cout << "path " << path << " uses " << rule.name() << "\n";
  if (count > 0)
    std::cout << count << " left\n";
  std::cerr << "done\n";
}

int loop(std::string text, int n) {
  std::string copy = text;
  while (n > 0)
    n = demo::tick(n);
  std::cout << copy << n;
  return n;
}
