-- Refunds and chargebacks take back the CPs of an order already credited
-- (payments.rs). What the character no longer has (spent already) becomes
-- `cps_debt`, paid off first by the CPs of its next purchases.
ALTER TABLE payment_orders ADD COLUMN IF NOT EXISTS reversed_at TIMESTAMPTZ;
-- The CPs taken back from the character; the rest of the order's went to its debt.
ALTER TABLE payment_orders ADD COLUMN IF NOT EXISTS reversed_cps INT;

ALTER TABLE characters ADD COLUMN IF NOT EXISTS cps_debt BIGINT NOT NULL DEFAULT 0 CHECK (cps_debt >= 0);

CREATE INDEX IF NOT EXISTS payment_orders_to_reverse
    ON payment_orders (order_id)
    WHERE status IN ('refunded', 'charged_back', 'cancelled') AND credited_at IS NOT NULL AND reversed_at IS NULL;
