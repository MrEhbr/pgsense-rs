-- Clean data: no sensitive patterns, exercises scanner overhead without regex hits
INSERT INTO bench_sensitive (full_name, email, phone, ssn, credit_card, notes)
VALUES (
    'User ' || :client_id,
    'no-match-here',
    '555-0100',
    'not-a-ssn',
    'not-a-cc',
    'Regular business notes for order processing run ' || :client_id
);
