-- Create separate databases for each service
-- This script runs on first initialization of the PostgreSQL container

CREATE DATABASE coordinator_db;
CREATE DATABASE node_a_db;
CREATE DATABASE node_b_db;

-- Grant the frost user full access to each database
GRANT ALL PRIVILEGES ON DATABASE coordinator_db TO frost;
GRANT ALL PRIVILEGES ON DATABASE node_a_db TO frost;
GRANT ALL PRIVILEGES ON DATABASE node_b_db TO frost;
