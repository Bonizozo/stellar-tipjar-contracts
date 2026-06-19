# Programmable Royalty System - Examples

## Example 1: Simple Two-Way Split

A music producer and vocalist collaborate on a track.

```rust
// Setup: Create rule for default split
let rule_id = programmable_royalty::create_rule(
    env,
    &creator_address,      // Channel/album owner
    &owner_address,        // Permission to modify
    vec![
        Condition::Always,  // Always apply this split
    ],
    vec![
        DynamicRecipient {
            recipient: producer_address,
            base_bps: 3000,  // 30%
            bonus_bps: 0,
        },
        DynamicRecipient {
            recipient: vocalist_address,
            base_bps: 7000,  // 70%
            bonus_bps: 0,
        },
    ],
    1,  // Priority
);

// Result: Every tip is split 30% to producer, 70% to vocalist
```

## Example 2: Dynamic Bonus for High Tips

Large supporters get VIP treatment with increased splits.

```rust
// Rule 1: Standard split for all tippers
programmable_royalty::create_rule(
    env,
    &channel,
    &manager,
    vec![Condition::Always],
    vec![
        DynamicRecipient {
            recipient: artist,
            base_bps: 8500,  // 85%
            bonus_bps: 0,
        },
        DynamicRecipient {
            recipient: platform,
            base_bps: 1500,  // 15%
            bonus_bps: 0,
        },
    ],
    2,  // Lower priority
);

// Rule 2: Higher artist cut for large tips (priority 1 = evaluated first)
programmable_royalty::create_rule(
    env,
    &channel,
    &manager,
    vec![
        Condition::MinAmount { threshold: 100_0000000 },  // >= 100 XLM
    ],
    vec![
        DynamicRecipient {
            recipient: artist,
            base_bps: 8500,
            bonus_bps: 500,  // +5% bonus = 90% total
        },
        DynamicRecipient {
            recipient: platform,
            base_bps: 1500,
            bonus_bps: -500,  // Reduced during bonus rule
        },
    ],
    1,  // Higher priority
);

// Behavior:
// - Tip < 100 XLM: 85%/15% split
// - Tip >= 100 XLM: 90%/10% split (bonus applied)
```

## Example 3: Time-Limited Campaign

Special promotions with time windows.

```rust
let campaign_start = 1700000000;  // Nov 15, 2023
let campaign_end = 1700604800;    // Nov 22, 2023

// Rule 1: Campaign period - special rates
programmable_royalty::create_rule(
    env,
    &creator,
    &admin,
    vec![
        Condition::TimeAfter { start_ts: campaign_start },
        Condition::TimeBefore { end_ts: campaign_end },
    ],
    vec![
        DynamicRecipient {
            recipient: artist,
            base_bps: 9500,  // 95% during campaign
            bonus_bps: 0,
        },
        DynamicRecipient {
            recipient: platform,
            base_bps: 500,   // 5%
            bonus_bps: 0,
        },
    ],
    2,
);

// Rule 2: Outside campaign - normal rates
programmable_royalty::create_rule(
    env,
    &creator,
    &admin,
    vec![
        Condition::Always,
    ],
    vec![
        DynamicRecipient {
            recipient: artist,
            base_bps: 8000,  // 80% normally
            bonus_bps: 0,
        },
        DynamicRecipient {
            recipient: platform,
            base_bps: 2000,  // 20%
            bonus_bps: 0,
        },
    ],
    1,
);
```

## Example 4: Whitelist VIP Treatment

Premium supporters receive special handling.

```rust
let vip_supporters = vec![
    Address::from_string("...", env),
    Address::from_string("...", env),
    Address::from_string("...", env),
];

// Rule 1: VIP tippers - extra rewards
programmable_royalty::create_rule(
    env,
    &creator,
    &owner,
    vec![
        Condition::FromList { senders: vip_supporters.clone() },
    ],
    vec![
        DynamicRecipient {
            recipient: artist,
            base_bps: 9000,  // 90%
            bonus_bps: 500,  // +5% bonus
        },
        DynamicRecipient {
            recipient: vip_bonus_pool,
            base_bps: 1000,  // 10% reserved for VIP perks
            bonus_bps: 0,
        },
    ],
    1,  // Higher priority
);

// Rule 2: Regular tippers
programmable_royalty::create_rule(
    env,
    &creator,
    &owner,
    vec![Condition::Always],
    vec![
        DynamicRecipient {
            recipient: artist,
            base_bps: 8500,  // 85%
            bonus_bps: 0,
        },
        DynamicRecipient {
            recipient: platform,
            base_bps: 1500,  // 15%
            bonus_bps: 0,
        },
    ],
    0,  // Lower priority
);
```

## Example 5: Milestone-Based Tiers

Artist gets better rates as they grow.

```rust
// Rule 1: Startup tier (< 100 total tips)
programmable_royalty::create_rule(
    env,
    &channel,
    &admin,
    vec![
        Condition::MinTotalTips { threshold: 0 },
        Condition::MaxTotalTips { threshold: 100 },  // Would need to add this
    ],
    vec![
        DynamicRecipient {
            recipient: artist,
            base_bps: 8000,  // 80%
            bonus_bps: 0,
        },
        DynamicRecipient {
            recipient: platform,
            base_bps: 2000,  // 20%
            bonus_bps: 0,
        },
    ],
    3,
);

// Rule 2: Growth tier (100-1000 tips)
programmable_royalty::create_rule(
    env,
    &channel,
    &admin,
    vec![
        Condition::MinTotalTips { threshold: 100 },
    ],
    vec![
        DynamicRecipient {
            recipient: artist,
            base_bps: 9000,  // 90%
            bonus_bps: 0,
        },
        DynamicRecipient {
            recipient: platform,
            base_bps: 1000,  // 10%
            bonus_bps: 0,
        },
    ],
    2,
);

// Rule 3: Success tier (1000+ tips)
programmable_royalty::create_rule(
    env,
    &channel,
    &admin,
    vec![
        Condition::MinTotalTips { threshold: 1000 },
    ],
    vec![
        DynamicRecipient {
            recipient: artist,
            base_bps: 9500,  // 95%
            bonus_bps: 0,
        },
        DynamicRecipient {
            recipient: platform,
            base_bps: 500,   // 5%
            bonus_bps: 0,
        },
    ],
    1,
);

// As creator accumulates more tips, the active rule automatically changes
```

## Example 6: Multi-Level Collaboration (Nested)

A producer's track samples original composer.

```rust
// Setup Original Composer
let original_artist = Address::from_string("...", env);
let original_rule = programmable_royalty::create_rule(
    env,
    &original_artist,
    &original_artist,
    vec![Condition::Always],
    vec![
        DynamicRecipient {
            recipient: original_artist,
            base_bps: 10000,  // Receives 100% of their share
            bonus_bps: 0,
        },
    ],
    1,
);

// Setup Producer with nested split
let producer_address = Address::from_string("...", env);
let producer_rule = programmable_royalty::create_rule(
    env,
    &producer_address,
    &producer_address,
    vec![Condition::Always],
    vec![
        DynamicRecipient {
            recipient: original_artist,  // Original gets royalty
            base_bps: 1500,  // 15% royalty
            bonus_bps: 0,
        },
        DynamicRecipient {
            recipient: producer_address,  // Producer keeps rest
            base_bps: 8500,  // 85%
            bonus_bps: 0,
        },
    ],
    1,
);

// Tip flow:
// Tip (100 XLM) to Producer
//   ├─ 15 XLM to original_artist (triggers their rule)
//   │   └─ 15 XLM stays with original_artist
//   └─ 85 XLM to producer
```

## Example 7: Author Royalty Chain

A book gets adapted into different formats, each paying upstream creators.

```rust
// Book -> Audiobook -> Podcast adaptation
// Each tier pays upstream royalties

// Original Book Author
programmable_royalty::create_rule(
    env,
    &original_author,
    &original_author,
    vec![Condition::Always],
    vec![
        DynamicRecipient {
            recipient: original_author,
            base_bps: 10000,
            bonus_bps: 0,
        },
    ],
    1,
);

// Audiobook Narrator (based on original)
programmable_royalty::create_rule(
    env,
    &audiobook_narrator,
    &audiobook_narrator,
    vec![Condition::Always],
    vec![
        DynamicRecipient {
            recipient: original_author,
            base_bps: 2000,  // 20% to original author
            bonus_bps: 0,
        },
        DynamicRecipient {
            recipient: audiobook_narrator,
            base_bps: 8000,  // 80% to narrator
            bonus_bps: 0,
        },
    ],
    1,
);

// Podcast Host (based on audiobook, which is based on original)
programmable_royalty::create_rule(
    env,
    &podcast_host,
    &podcast_host,
    vec![Condition::Always],
    vec![
        DynamicRecipient {
            recipient: audiobook_narrator,  // Narrator of adapter gets cut
            base_bps: 1000,  // 10% up the chain
            bonus_bps: 0,
        },
        DynamicRecipient {
            recipient: podcast_host,
            base_bps: 9000,  // 90% to podcast
            bonus_bps: 0,
        },
    ],
    1,
);

// Flow (assuming recursion limits):
// Tip (100 XLM) to Podcast Host
//   ├─ 10 XLM to Narrator → 2 XLM to Author, 8 XLM to Narrator
//   └─ 90 XLM to Podcast Host
```

## Example 8: Conditional Special Handler

High-value tippers get personalized service.

```rust
// Rule 1: VIP high-value tippers
programmable_royalty::create_rule(
    env,
    &creator,
    &owner,
    vec![
        Condition::MinAmount { threshold: 500_0000000 },  // >= 500 XLM
        Condition::FromList { 
            senders: vec![
                Address::from_string("vip1", env),
                Address::from_string("vip2", env),
            ],
        },
    ],
    vec![
        DynamicRecipient {
            recipient: artist,
            base_bps: 9200,  // 92% to artist
            bonus_bps: 0,
        },
        DynamicRecipient {
            recipient: concierge_service,
            base_bps: 800,   // 8% for personal thank-you service
            bonus_bps: 0,
        },
    ],
    1,
);

// Rule 2: Standard large tips
programmable_royalty::create_rule(
    env,
    &creator,
    &owner,
    vec![
        Condition::MinAmount { threshold: 100_0000000 },  // >= 100 XLM
    ],
    vec![
        DynamicRecipient {
            recipient: artist,
            base_bps: 9000,  // 90%
            bonus_bps: 0,
        },
        DynamicRecipient {
            recipient: platform,
            base_bps: 1000,  // 10%
            bonus_bps: 0,
        },
    ],
    2,
);

// Rule 3: Default for all other tips
programmable_royalty::create_rule(
    env,
    &creator,
    &owner,
    vec![Condition::Always],
    vec![
        DynamicRecipient {
            recipient: artist,
            base_bps: 8500,
            bonus_bps: 0,
        },
        DynamicRecipient {
            recipient: platform,
            base_bps: 1500,
            bonus_bps: 0,
        },
    ],
    3,
);
```

## API Usage Patterns

### Creating Rules Programmatically

```rust
use crate::programmable_royalty::{self, Condition, DynamicRecipient};

fn setup_collaboration_rules(env: &Env, creator: &Address) {
    // Multiple recipients with conditions
    let rule_id = programmable_royalty::create_rule(
        env,
        creator,
        creator,
        vec![
            Condition::MinAmount { threshold: 50_0000000 },
        ],
        vec![
            DynamicRecipient {
                recipient: creator_address,
                base_bps: 6000,
                bonus_bps: 1000,
            },
            DynamicRecipient {
                recipient: collaborator_address,
                base_bps: 4000,
                bonus_bps: 0,
            },
        ],
        1,
    );
}
```

### Querying Rules and History

```rust
// Get active rules for a creator
let rules = programmable_royalty::get_creator_rules(env, &creator);

// Check total tips received
let total_tips = programmable_royalty::get_total_tips_received(env, &creator);

// Get payment history (last 10 payments)
let count = programmable_royalty::get_payment_count(env, &creator);
for i in (count.saturating_sub(10))..count {
    if let Some(payment) = programmable_royalty::get_payment(env, &creator, i) {
        println!("Paid {} to {} via rule {}", 
            payment.amount, 
            payment.recipient, 
            payment.rule_id);
    }
}

// Check accumulated balance
let balance = programmable_royalty::get_programmable_balance(env, &recipient);
```

### Managing Rules

```rust
// Update a rule's conditions or recipients
programmable_royalty::update_rule(
    env,
    &creator,
    rule_id,
    &owner,
    new_conditions,
    new_recipients,
    new_priority,
);

// Disable a rule temporarily (keeping it for reference)
programmable_royalty::toggle_rule(env, rule_id, &owner, false);

// Re-enable it
programmable_royalty::toggle_rule(env, rule_id, &owner, true);
```
