#!/bin/bash

# Security check script for TipJar contract

set -e

echo "=== TipJar Security Check ==="
echo ""

# Check for unsafe code
echo "Checking for unsafe code..."
if grep -r "unsafe" contracts/tipjar/src --include="*.rs" | grep -v "deny(unsafe_code)"; then
    echo "❌ Unsafe code found!"
    exit 1
else
    echo "✓ No unsafe code detected"
fi

# Check for missing documentation
echo ""
echo "Checking for missing documentation..."
if grep -r "pub fn" contracts/tipjar/src/lib.rs | grep -v "///" | head -5; then
    echo "⚠ Some public functions may lack documentation"
else
    echo "✓ Documentation check passed"
fi

# Run security tests
echo ""
echo "Running security tests..."
cargo test -p tipjar --test security_tests --lib 2>&1 | tail -20

# Check for common vulnerabilities
echo ""
echo "Checking for common vulnerability patterns..."

# Check for unchecked arithmetic
if grep -r "checked_add\|checked_sub\|checked_mul" contracts/tipjar/src --include="*.rs" > /dev/null; then
    echo "✓ Checked arithmetic operations found"
else
    echo "⚠ Consider using checked arithmetic for critical operations"
fi

# Check for require_auth calls
if grep -r "require_auth" contracts/tipjar/src --include="*.rs" > /dev/null; then
    echo "✓ Authorization checks found"
else
    echo "⚠ Verify all state-changing functions have authorization"
fi

echo ""
echo "=== Security Check Complete ==="
