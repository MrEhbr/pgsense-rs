-- Sensitive data: every text column contains detectable patterns
INSERT INTO bench_sensitive (full_name, email, phone, ssn, credit_card, notes)
VALUES (
    'John Smith',
    'john.smith' || :client_id || '@example.com',
    '555-123-4567',
    '123-45-6789',
    '4111111111111111',
    'Contact SSN 987-65-4320 email test' || :client_id || '@corp.com'
);
