// Saan ARPG Enemy AI: Shadow Stalker Chase & Melee Pacing
// Runs on each enemy unit per tick

fn on_tick(ctx) {
    // Dynamically chase the hero player using ctx.target_pos
    let diff = ctx.target_pos - ctx.pos;
    diff.y = 0.0;
    let dist = diff.length();

    if dist > 1.2 && dist < 14.0 {
        // Chase hero towards player position
        let dir = diff.normalize();
        let speed = 2.6;
        ctx.set_vel(dir.x * speed, 0.0, dir.z * speed);
    } else if dist <= 1.2 {
        // In strike range: stop and ready attack
        ctx.set_vel(0.0, 0.0, 0.0);
        ctx.emit("enemy_melee_contact");
    } else {
        // Outside detection range
        ctx.set_vel(0.0, 0.0, 0.0);
    }

    return ctx;
}

fn on_event(ctx, tag) {
    if tag.starts_with("combat_hit") {
        saan_log("Shadow Stalker took damage and recoiled!");
        ctx.emit("audio:audio/blade_impact.ogg");
    }
    return ctx;
}
