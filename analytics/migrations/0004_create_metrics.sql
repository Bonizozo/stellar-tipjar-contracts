-- Create tips table for metrics collection
CREATE TABLE IF NOT EXISTS tips (
    id VARCHAR(255) PRIMARY KEY,
    sender VARCHAR(255) NOT NULL,
    creator VARCHAR(255) NOT NULL,
    amount BIGINT NOT NULL,
    token VARCHAR(255) NOT NULL,
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create withdrawals table
CREATE TABLE IF NOT EXISTS withdrawals (
    id VARCHAR(255) PRIMARY KEY,
    creator VARCHAR(255) NOT NULL,
    amount BIGINT NOT NULL,
    token VARCHAR(255) NOT NULL,
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create metrics cache table
CREATE TABLE IF NOT EXISTS metrics_cache (
    date DATE PRIMARY KEY,
    data JSONB NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_tips_creator ON tips(creator);
CREATE INDEX IF NOT EXISTS idx_tips_sender ON tips(sender);
CREATE INDEX IF NOT EXISTS idx_tips_timestamp ON tips(timestamp);
CREATE INDEX IF NOT EXISTS idx_tips_date ON tips(DATE(timestamp));
CREATE INDEX IF NOT EXISTS idx_withdrawals_creator ON withdrawals(creator);
CREATE INDEX IF NOT EXISTS idx_withdrawals_timestamp ON withdrawals(timestamp);

-- Create view for daily aggregates
CREATE OR REPLACE VIEW daily_metrics AS
SELECT 
    DATE(timestamp) as date,
    COUNT(*) as tip_count,
    COALESCE(SUM(amount), 0) as total_volume,
    COUNT(DISTINCT sender) as unique_tippers,
    COUNT(DISTINCT creator) as unique_creators,
    AVG(amount) as average_tip
FROM tips
GROUP BY DATE(timestamp)
ORDER BY date DESC;

-- Create view for creator leaderboard
CREATE OR REPLACE VIEW creator_leaderboard AS
SELECT 
    creator,
    SUM(amount) as total_received,
    COUNT(*) as tip_count,
    AVG(amount) as average_tip,
    MAX(timestamp) as last_tip
FROM tips
GROUP BY creator
ORDER BY total_received DESC;

-- Create view for tipper leaderboard
CREATE OR REPLACE VIEW tipper_leaderboard AS
SELECT 
    sender,
    SUM(amount) as total_sent,
    COUNT(*) as tip_count,
    AVG(amount) as average_tip,
    MAX(timestamp) as last_tip
FROM tips
GROUP BY sender
ORDER BY total_sent DESC;
