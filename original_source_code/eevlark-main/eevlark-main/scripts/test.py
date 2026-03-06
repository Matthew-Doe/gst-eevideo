# basic_starlark_test.star
# Tests only built-in Starlark features (no custom builtins required)

print("Basic Starlark smoke test – started")

# 1. Basic types and literals
i = 42
f = 3.14159
s = "Hello, Starlark!"
b = True
none = None
lst = [1, 2, 3, "four"]
tup = (10, 20, "thirty")
dct = {"a": 1, "b": 2, "nested": {"x": 99}}

print("Types created:")
print("  int:   ", i)
print("  float: ", f)
print("  str:   ", s)
print("  bool:  ", b)
print("  None:  ", none)
print("  list:  ", lst)
print("  tuple: ", tup)
print("  dict:  ", dct)

# 2. Simple arithmetic & comparisons
sum_val = i + 8
prod = i * 10
div = 100 / 4
mod = 17 % 5

print("\nArithmetic:")
print("  42 + 8       =", sum_val)
print("  42 * 10      =", prod)
print("  100 / 4      =", div)
print("  17 % 5       =", mod)
print("  Comparisons: ", 10 < 20, 30 > 100, 5 == 5, "a" != "b")

# 3. List & dict operations
lst.append(999)
lst.pop(0)              # remove first element
dct["new"] = "value"
dct.pop("a")

print("\nAfter mutations:")
print("  modified list:", lst)
print("  modified dict:", dct)

# 4. Loops & comprehensions
squares = [x*x for x in range(6)]
evens = [x for x in range(10) if x % 2 == 0]

print("\nComprehensions:")
print("  squares 0..5:", squares)
print("  evens 0..9:  ", evens)

# 5. Conditionals & functions
def describe(n):
    if n < 0:
        return "negative"
    elif n == 0:
        return "zero"
    else:
        return "positive"

print("\nFunction calls:")
print("  describe(-3):", describe(-3))
print("  describe(0): ", describe(0))
print("  describe(7): ", describe(7))

# 6. String operations
greeting = "Starlark " + "is " + "fun"
upper = greeting.upper()
split_words = greeting.split(" ")

print("\nStrings:")
print("  concat:   ", greeting)
print("  upper:    ", upper)
print("  split:    ", split_words)

print("\nAll basic tests completed successfully ✓")