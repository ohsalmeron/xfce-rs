#!/bin/bash
# Health monitoring script for xfwm4-rs

LOG_FILE="logs/xfwm4-rs.log"

echo "=== xfwm4-rs Health Check ==="
echo "Time: $(date)"
echo ""

# Check if log file exists
if [ ! -f "$LOG_FILE" ]; then
    echo "ERROR: Log file not found at $LOG_FILE"
    exit 1
fi

# Count critical errors
ERRORS=$(grep -c "ERROR" "$LOG_FILE" 2>/dev/null | tail -1)
WARNINGS=$(grep -c "WARN" "$LOG_FILE" 2>/dev/null | tail -1)

# Default to 0 if grep fails
ERRORS=${ERRORS:-0}
WARNINGS=${WARNINGS:-0}

echo "Error Count: $ERRORS"
echo "Warning Count: $WARNINGS"
echo ""

# Show extension versions
echo "=== Extension Versions ==="
grep "extension v" "$LOG_FILE" | tail -4
echo ""

# Check for recent errors
echo "=== Recent Errors (last 10) ==="
grep "ERROR" "$LOG_FILE" | tail -10
echo ""

# Check for recent warnings
echo "=== Recent Warnings (last 10) ==="
grep "WARN" "$LOG_FILE" | tail -10
echo ""

# Check compositor status
echo "=== Compositor Status ==="
if grep -q "Compositor enabled" "$LOG_FILE"; then
    echo "✓ Compositor: ENABLED"
else
    echo "✗ Compositor: FAILED TO ENABLE"
fi

# Check managed windows
echo ""
echo "=== Managed Windows ==="
grep "Managing window" "$LOG_FILE" | tail -5
echo ""

# Health verdict
echo "=== Health Verdict ==="
if [ "$ERRORS" -gt 10 ]; then
    echo "🔴 UNHEALTHY: Too many errors ($ERRORS > 10)"
    exit 1
elif [ "$ERRORS" -gt 5 ]; then
    echo "🟡 DEGRADED: Errors present ($ERRORS)"
    exit 2
else
    echo "🟢 HEALTHY: System operational"
    exit 0
fi
