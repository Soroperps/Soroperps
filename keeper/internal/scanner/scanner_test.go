package scanner

import (
	"testing"
)

func TestAddAndGetPositions(t *testing.T) {
	s := New(nil, nil, 0) // client/contractIDs not needed for unit tests

	s.AddPosition("GTRADER1", PositionInfo{
		PositionID: 1,
		Trader:     "GTRADER1",
		Asset:      0,
		Direction:  "long",
		Size:       1000,
		Collateral: 100,
		EntryPrice: 1500000,
	})

	s.AddPosition("GTRADER1", PositionInfo{
		PositionID: 2,
		Trader:     "GTRADER1",
		Asset:      0,
		Direction:  "short",
		Size:       500,
		Collateral: 50,
		EntryPrice: 1500000,
	})

	s.AddPosition("GTRADER2", PositionInfo{
		PositionID: 3,
		Trader:     "GTRADER2",
		Asset:      1,
		Direction:  "long",
		Size:       2000,
		Collateral: 200,
		EntryPrice: 50000000000,
	})

	positions := s.GetOpenPositions()

	if len(positions) != 2 {
		t.Errorf("expected 2 traders, got %d", len(positions))
	}
	if len(positions["GTRADER1"]) != 2 {
		t.Errorf("expected 2 positions for GTRADER1, got %d", len(positions["GTRADER1"]))
	}
	if len(positions["GTRADER2"]) != 1 {
		t.Errorf("expected 1 position for GTRADER2, got %d", len(positions["GTRADER2"]))
	}
	if s.GetPositionCount() != 3 {
		t.Errorf("expected total count 3, got %d", s.GetPositionCount())
	}
}

func TestRemovePosition(t *testing.T) {
	s := New(nil, nil, 0)

	s.AddPosition("GTRADER1", PositionInfo{PositionID: 1, Trader: "GTRADER1"})
	s.AddPosition("GTRADER1", PositionInfo{PositionID: 2, Trader: "GTRADER1"})

	s.RemovePosition("GTRADER1", 1)

	positions := s.GetOpenPositions()
	if len(positions["GTRADER1"]) != 1 {
		t.Errorf("expected 1 position after removal, got %d", len(positions["GTRADER1"]))
	}
	if positions["GTRADER1"][0].PositionID != 2 {
		t.Errorf("expected position 2 to remain, got %d", positions["GTRADER1"][0].PositionID)
	}
}

func TestRemoveLastPosition(t *testing.T) {
	s := New(nil, nil, 0)

	s.AddPosition("GTRADER1", PositionInfo{PositionID: 1, Trader: "GTRADER1"})
	s.RemovePosition("GTRADER1", 1)

	positions := s.GetOpenPositions()
	if len(positions) != 0 {
		t.Errorf("expected empty map after removing last position, got %d traders", len(positions))
	}
	if s.GetPositionCount() != 0 {
		t.Errorf("expected count 0, got %d", s.GetPositionCount())
	}
}

func TestRemoveNonexistent(t *testing.T) {
	s := New(nil, nil, 0)

	s.AddPosition("GTRADER1", PositionInfo{PositionID: 1, Trader: "GTRADER1"})

	// Should not panic
	s.RemovePosition("GTRADER1", 999)
	s.RemovePosition("NONEXISTENT", 1)

	if s.GetPositionCount() != 1 {
		t.Errorf("expected count 1 after no-op removals, got %d", s.GetPositionCount())
	}
}

func TestGetOpenPositionsIsCopy(t *testing.T) {
	s := New(nil, nil, 0)

	s.AddPosition("GTRADER1", PositionInfo{PositionID: 1, Trader: "GTRADER1"})

	// Get a copy and modify it
	positions := s.GetOpenPositions()
	positions["GTRADER1"] = append(positions["GTRADER1"], PositionInfo{PositionID: 99})

	// Original should be unchanged
	if s.GetPositionCount() != 1 {
		t.Errorf("modifying copy affected original, count = %d", s.GetPositionCount())
	}
}
