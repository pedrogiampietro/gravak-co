-- Conquer Points bought with real money through Mercado Pago: by card or Pix
-- on the website, by Pix in the client. The website creates the orders and
-- marks them approved once Mercado Pago confirms the payment; the game server
-- credits the CPs to the character (payments.rs), once per order.
CREATE TABLE IF NOT EXISTS cp_packages (
    package_id INT PRIMARY KEY,
    price_cents INT NOT NULL CHECK (price_cents > 0),
    cps INT NOT NULL CHECK (cps > 0),
    active BOOLEAN NOT NULL DEFAULT TRUE
);

-- R$ 0,50 a CP, with a bonus that grows with the package.
INSERT INTO cp_packages (package_id, price_cents, cps) VALUES
    (1, 1000, 20),
    (2, 2500, 55),
    (3, 5000, 120),
    (4, 10000, 260),
    (5, 20000, 550)
ON CONFLICT (package_id) DO NOTHING;

-- The price and CPs are copied from the package, so changing a package never
-- changes an order already made. Orders outlive their account or character:
-- they are the record of what was paid.
CREATE TABLE IF NOT EXISTS payment_orders (
    order_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id BIGINT REFERENCES accounts (account_id) ON DELETE SET NULL,
    character_id BIGINT REFERENCES characters (character_id) ON DELETE SET NULL,
    character_name TEXT NOT NULL,
    package_id INT NOT NULL REFERENCES cp_packages (package_id),
    price_cents INT NOT NULL CHECK (price_cents > 0),
    cps INT NOT NULL CHECK (cps > 0),
    -- Where it was bought: 'site' (Checkout Pro) or 'client' (Pix).
    channel TEXT NOT NULL CHECK (channel IN ('site', 'client')),
    -- Mercado Pago's status of the payment: pending, approved, rejected,
    -- cancelled, refunded, charged_back...
    status TEXT NOT NULL DEFAULT 'pending',
    mp_preference_id TEXT,
    mp_payment_id BIGINT UNIQUE,
    -- The Pix code and its QR image, for the client to show again.
    pix_code TEXT,
    pix_qr_base64 TEXT,
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    paid_at TIMESTAMPTZ,
    credited_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS payment_orders_to_credit
    ON payment_orders (paid_at) WHERE status = 'approved' AND credited_at IS NULL;
CREATE INDEX IF NOT EXISTS payment_orders_by_account ON payment_orders (account_id, created_at DESC);
CREATE INDEX IF NOT EXISTS payment_orders_by_character ON payment_orders (character_id, created_at DESC);
