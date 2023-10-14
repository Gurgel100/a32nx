/* eslint-disable max-len */
import React, { useCallback, useEffect, useState } from 'react';
import { round } from 'lodash';
import { CloudArrowDown, PlayFill, StopCircleFill } from 'react-bootstrap-icons';
import { useSimVar, usePersistentNumberProperty, usePersistentProperty, Units } from '@flybywiresim/fbw-sdk';
// import Slider from 'rc-slider';
import Card from 'instruments/src/EFB/UtilComponents/Card/Card';
import { A380FuelOutline } from 'instruments/src/EFB/Assets/FuelOutline';
import Slider from 'rc-slider';
import { t } from '../../../../translation';
import { TooltipWrapper } from '../../../../UtilComponents/TooltipWrapper';
import { SimpleInput } from '../../../../UtilComponents/Form/SimpleInput/SimpleInput';
import { SelectGroup, SelectItem } from '../../../../UtilComponents/Form/Select';
import { ProgressBar } from '../../../../UtilComponents/Progress/Progress';

// TODO: Page is very WIP, needs to be cleaned up and refactored

interface ValueInputProps {
    min: number,
    max: number,
    value: number
    onBlur: (v: string) => void,
    unit: string,
    disabled?: boolean
}

const ValueInput: React.FC<ValueInputProps> = ({ min, max, value, onBlur, unit, disabled }) => (
    <div className="relative w-40">
        <SimpleInput
            className={`my-2 w-full font-mono ${(disabled ? 'cursor-not-allowed placeholder-theme-body text-theme-body' : '')}`}
            fontSizeClassName="text-2xl"
            number
            min={min}
            max={max}
            value={value.toFixed(0)}
            onBlur={onBlur}
        />
        <div className="flex absolute top-0 right-3 items-center h-full font-mono text-2xl text-gray-400">{unit}</div>
    </div>
);

interface ValueSimbriefInputProps {
    min: number,
    max: number,
    value: number
    onBlur: (v: string) => void,
    unit: string,
    showSimbriefButton: boolean,
    onClickSync: () => void,
    disabled?: boolean
}

const ValueSimbriefInput: React.FC<ValueSimbriefInputProps> = ({ min, max, value, onBlur, unit, showSimbriefButton, onClickSync, disabled }) => (
    <div className="relative w-52">
        <div className="flex flex-row">
            <div className="relative">
                <SimpleInput
                    className={`${showSimbriefButton && 'rounded-r-none'} my-2 w-full font-mono ${(disabled ? 'cursor-not-allowed placeholder-theme-body text-theme-body' : '')}`}
                    fontSizeClassName="text-2xl"
                    number
                    min={min}
                    max={max}
                    value={value.toFixed(0)}
                    onBlur={onBlur}
                />
                <div className="flex absolute top-0 right-3 items-center h-full font-mono text-2xl text-gray-400">{unit}</div>
            </div>
            {showSimbriefButton
                && (
                    <TooltipWrapper text={t('Ground.Payload.TT.FillPayloadFromSimbrief')}>
                        <div
                            className={`flex justify-center items-center my-2 px-2 h-auto text-theme-body
                                        hover:text-theme-highlight bg-theme-highlight hover:bg-theme-body
                                        rounded-md rounded-l-none border-2 border-theme-highlight transition duration-100`}
                            onClick={onClickSync}
                        >
                            <CloudArrowDown size={26} />
                        </div>
                    </TooltipWrapper>
                )}
        </div>
    </div>
);

interface NumberUnitDisplayProps {
    /**
     * The value to show
     */
    value: number,

    /**
     * The amount of leading zeroes to pad with
     */
    padTo: number,

    /**
     * The unit to show at the end
     */
    unit: string,
}

const ValueUnitDisplay: React.FC<NumberUnitDisplayProps> = ({ value, padTo, unit }) => {
    const fixedValue = value.toFixed(0);
    const leadingZeroCount = Math.max(0, padTo - fixedValue.length);

    return (
        <span className="flex items-center">
            <span className="flex justify-end pr-2 text-2xl">
                <span className="text-2xl text-gray-600">{'0'.repeat(leadingZeroCount)}</span>
                {fixedValue}
            </span>
            {' '}
            <span className="text-2xl text-gray-500">{unit}</span>
        </span>
    );
};

interface FuelProps {
    simbriefDataLoaded: boolean,
    simbriefPlanRamp: number,
    simbriefUnits: string,
    massUnitForDisplay: string,
    isOnGround: boolean,
}
export const A380Fuel: React.FC<FuelProps> = ({
    simbriefDataLoaded,
    simbriefPlanRamp,
    simbriefUnits,
    massUnitForDisplay,
    isOnGround,
}) => {
    const [TOTAL_FUEL_GALLONS] = useState(85471.7); // 323545.6 litres
    const [FUEL_GALLONS_TO_KG] = useState(3.039075693483925);
    const [TOTAL_MAX_FUEL_KG] = useState(TOTAL_FUEL_GALLONS * FUEL_GALLONS_TO_KG);

    const [eng1Running] = useSimVar('ENG COMBUSTION:1', 'Bool', 1_000);
    const [eng4Running] = useSimVar('ENG COMBUSTION:4', 'Bool', 1_000);
    const [refuelRate, setRefuelRate] = usePersistentProperty('REFUEL_RATE_SETTING');

    const [INNER_FEED_MAX_KG] = useState(7753.2 * FUEL_GALLONS_TO_KG); // 23562.51 kg - 47053.02
    const [OUTER_FEED_MAX_KG] = useState(7299.6 * FUEL_GALLONS_TO_KG); // 22189.04 kg - 44378.08
    const [INNER_TANK_MAX_KG] = useState(12189.4 * FUEL_GALLONS_TO_KG); // 37044.51 kg
    const [MID_TANK_MAX_KG] = useState(9632 * FUEL_GALLONS_TO_KG); // 29299.51 kg
    const [OUTER_TANK_MAX_KG] = useState(2731.5 * FUEL_GALLONS_TO_KG); // 8299.51 kg
    const [TRIM_TANK_MAX_KG] = useState(6260.3 * FUEL_GALLONS_TO_KG); // 18999.51 kg

    const [POINT_A_KG] = useState(18000);
    const [POINT_B_KG] = useState(26000);
    const [POINT_C_KG] = useState(36000);
    const [POINT_D_KG] = useState(47000);
    const [POINT_E_KG] = useState(103788);
    const [POINT_F_KG] = useState(158042);
    const [POINT_G_KG] = useState(215702);
    const [POINT_H_KG] = useState(223028);

    const [FEED_A_KG] = useState(4500);
    const [FEED_C_KG] = useState(7000);
    const [OUTER_FEED_E_KG] = useState(20558);
    const [INNER_FEED_E_KG] = useState(21836);

    const [OUTER_TANK_B_KG] = useState(4000);
    const [OUTER_TANK_H_KG] = useState(7693);
    const [MID_TANK_F_KG] = useState(27127);
    const [INNER_TANK_D_KG] = useState(5500);
    const [INNER_TANK_G_KG] = useState(34300);

    // TODO: Remove and implement proper fueling logic with fueling backend in rust (do not use A32NX_Refuel.js!!!)
    const [leftOuterGal, setLeftOuter] = useSimVar('FUELSYSTEM TANK QUANTITY:1', 'Gallons', 2_000); // 2731.5
    const [feedOneGal, setFeedOne] = useSimVar('FUELSYSTEM TANK QUANTITY:2', 'Gallons', 2_000); //  7299.6
    const [leftMidGal, setLeftMid] = useSimVar('FUELSYSTEM TANK QUANTITY:3', 'Gallons', 2_000); // 9632
    const [leftInnerGal, setLeftInner] = useSimVar('FUELSYSTEM TANK QUANTITY:4', 'Gallons', 2_000); // 12189.4
    const [feedTwoGal, setFeedTwo] = useSimVar('FUELSYSTEM TANK QUANTITY:5', 'Gallons', 2_000); // 7753.2
    const [feedThreeGal, setFeedThree] = useSimVar('FUELSYSTEM TANK QUANTITY:6', 'Gallons', 2_000); // 7753.2
    const [rightInnerGal, setRightInner] = useSimVar('FUELSYSTEM TANK QUANTITY:7', 'Gallons', 2_000); // 12189.4
    const [rightMidGal, setRightMid] = useSimVar('FUELSYSTEM TANK QUANTITY:8', 'Gallons', 2_000); // 9632
    const [feedFourGal, setFeedFour] = useSimVar('FUELSYSTEM TANK QUANTITY:9', 'Gallons', 2_000); // 7299.6
    const [rightOuterGal, setRightOuter] = useSimVar('FUELSYSTEM TANK QUANTITY:10', 'Gallons', 2_000); // 2731.5
    const [trimGal, setTrim] = useSimVar('FUELSYSTEM TANK QUANTITY:11', 'Gallons', 2_000); // 6260.3
    const [totalFuelWeightKg] = useSimVar('FUEL TOTAL QUANTITY WEIGHT', 'Kilograms', 500); // 6260.3

    // TODO: Remove debug override

    // TODO: Remove
    const [fuelDesiredPercent, setFuelDesiredPercent] = useSimVar('L:A32NX_FUEL_DESIRED_PERCENT', 'Number');
    const [fuelDesired, setFuelDesired] = useSimVar('L:A32NX_FUEL_DESIRED', 'Kilograms');
    const [refuelStartedByUser, setRefuelStartedByUser] = useSimVar('L:A32NX_REFUEL_STARTED_BY_USR', 'Bool');

    // Simbrief
    const [showSimbriefButton, setShowSimbriefButton] = useState(false);

    // GSX
    const [gsxFuelSyncEnabled] = usePersistentNumberProperty('GSX_FUEL_SYNC', 0);
    const [gsxFuelHoseConnected] = useSimVar('L:FSDT_GSX_FUELHOSE_CONNECTED', 'Number');

    useEffect(() => {
        // GSX
        if (gsxFuelSyncEnabled === 1) {
            /*
            if (boardingStarted) {
                setShowSimbriefButton(false);
                return;
            }
            */
            setShowSimbriefButton(simbriefDataLoaded);
            return;
        }
        // EFB
        if (Math.abs(Math.round(totalFuelWeightKg) - roundUpNearest100(simbriefPlanRamp)) < 10) {
            setShowSimbriefButton(false);
            return;
        }
        setShowSimbriefButton(simbriefDataLoaded);
    }, [totalFuelWeightKg, simbriefDataLoaded]);

    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    const airplaneCanRefuel = () => {
        if (refuelRate !== '2') {
            if (eng1Running || eng4Running || !isOnGround) {
                setRefuelRate('2');
            }
        }

        if (gsxFuelSyncEnabled === 1) {
            if (gsxFuelHoseConnected === 1) {
                return true;
            }

            // In-flight refueling with GSX Sync enabled
            return (eng1Running || eng4Running || !isOnGround) && refuelRate === '2';
        }
        return true;
    };

    const formatFuelFilling = (curr: number, max: number) => {
        const percent = (Math.max(curr, 0) / max) * 100;
        return `linear-gradient(to top, var(--color-highlight) ${percent}%,#ffffff00 0%)`;
    };

    // TODO: Replace with proper FQMS system (in rust systems)
    const calculateDesiredFuelKg = useCallback((fuelWeightKg: number) => {
        let wingKg = 0;
        let trimKg = 0;
        // Temporary code
        if (fuelWeightKg <= POINT_E_KG) {
            wingKg = fuelWeightKg;
        } else if (fuelWeightKg <= POINT_F_KG) {
            trimKg = Math.min(4000, fuelWeightKg - POINT_E_KG);
        } else if (fuelWeightKg <= POINT_H_KG) {
            trimKg = Math.max(Math.min(8000, fuelWeightKg - POINT_G_KG), 4000);
        } else {
            trimKg = Math.max(Math.min(TRIM_TANK_MAX_KG, fuelWeightKg - POINT_H_KG), 8000);
        }
        setTrim(trimKg / FUEL_GALLONS_TO_KG);

        wingKg = fuelWeightKg - trimKg;

        let outerFeedKg = 0;
        let innerFeedKg = 0;
        let outerTankKg = 0;
        let midTankKg = 0;
        let innerTankKg = 0;
        if (wingKg <= POINT_A_KG) {
            outerFeedKg = (wingKg / 4);
            innerFeedKg = (wingKg / 4);
        } else if (wingKg <= POINT_B_KG) {
            outerFeedKg = FEED_A_KG;
            innerFeedKg = FEED_A_KG;
            outerTankKg = (wingKg - POINT_A_KG) / 2;
        } else if (wingKg <= POINT_C_KG) {
            outerFeedKg = FEED_A_KG + (wingKg - POINT_B_KG) / 4;
            innerFeedKg = FEED_A_KG + (wingKg - POINT_B_KG) / 4;
            outerTankKg = OUTER_TANK_B_KG;
        } else if (wingKg <= POINT_D_KG) {
            outerFeedKg = FEED_C_KG;
            innerFeedKg = FEED_C_KG;
            outerTankKg = OUTER_TANK_B_KG;
            innerTankKg = (wingKg - POINT_C_KG) / 2;
        } else if (wingKg <= POINT_E_KG) {
            const totalFeedE = (OUTER_FEED_E_KG * 2 + INNER_FEED_E_KG * 2);
            outerFeedKg = FEED_C_KG + (wingKg - POINT_D_KG) * (OUTER_FEED_E_KG / totalFeedE);
            innerFeedKg = FEED_C_KG + (wingKg - POINT_D_KG) * (INNER_FEED_E_KG / totalFeedE);
            outerTankKg = OUTER_TANK_B_KG;
            innerTankKg = INNER_TANK_D_KG;
        } else if (wingKg <= POINT_F_KG) {
            outerFeedKg = OUTER_FEED_E_KG;
            innerFeedKg = INNER_FEED_E_KG;
            outerTankKg = OUTER_TANK_B_KG;
            innerTankKg = INNER_TANK_D_KG;
            midTankKg = (wingKg - POINT_E_KG) / 2;
        } else if (wingKg <= POINT_G_KG) {
            outerFeedKg = OUTER_FEED_E_KG;
            innerFeedKg = INNER_FEED_E_KG;
            outerTankKg = OUTER_TANK_B_KG;
            innerTankKg = INNER_TANK_D_KG + (wingKg - POINT_F_KG) / 2;
            midTankKg = MID_TANK_F_KG;
        } else if (wingKg <= POINT_H_KG) {
            outerFeedKg = OUTER_FEED_E_KG;
            innerFeedKg = INNER_FEED_E_KG;
            outerTankKg = OUTER_TANK_B_KG + (wingKg - POINT_G_KG) / 2;
            innerTankKg = INNER_TANK_G_KG;
            midTankKg = MID_TANK_F_KG;
        } else {
            outerFeedKg = OUTER_FEED_E_KG + (wingKg - POINT_H_KG) / 10;
            innerFeedKg = INNER_FEED_E_KG + (wingKg - POINT_H_KG) / 10;
            outerTankKg = OUTER_TANK_H_KG + (wingKg - POINT_H_KG) / 10;
            innerTankKg = INNER_TANK_G_KG + (wingKg - POINT_H_KG) / 10;
            midTankKg = MID_TANK_F_KG + (wingKg - POINT_H_KG) / 10;
        }

        setFeedOne(outerFeedKg / FUEL_GALLONS_TO_KG);
        setFeedFour(outerFeedKg / FUEL_GALLONS_TO_KG);
        setFeedTwo(innerFeedKg / FUEL_GALLONS_TO_KG);
        setFeedThree(innerFeedKg / FUEL_GALLONS_TO_KG);
        setLeftOuter(outerTankKg / FUEL_GALLONS_TO_KG);
        setRightOuter(outerTankKg / FUEL_GALLONS_TO_KG);
        setLeftInner(innerTankKg / FUEL_GALLONS_TO_KG);
        setRightInner(innerTankKg / FUEL_GALLONS_TO_KG);
        setLeftMid(midTankKg / FUEL_GALLONS_TO_KG);
        setRightMid(midTankKg / FUEL_GALLONS_TO_KG);

        /*
        Feed 1+4 Tanks
        F14 = (WT / 4) if WT <= [Point A]
        F14 = [Outer Feed Mark A] if [Point B] >= WT > [Point A]
        F14 = [Outer Feed Mark A] + (WT - [Point B]) / 4 if [Point C] >= WT > [Point B]
        F14 = [Outer Feed Mark C] if [Point D] >= WT > [Point C]
        F14 = [Outer Feed Mark C] + (WT - [Point D]) * ([Outer Feed Mark E]/([Outer Feed Mark E] * 2 + [Inner Feed Mark E] * 2)) if [Point E] >= WT > [Point D]
        F14 = [Outer Feed Mark E] if [Point H] >= WT > [Point E]
        F14 = [Outer Feed Mark E] + (WT - [Point H]) / 10 if WT > [Point H]

        Feed 2+3 Tanks
        F23 = (WT / 4) if WT <= [Point A]
        F23 = [Inner Feed Mark A] if [Point B] >= WT > [Point A]
        F23 = [Inner Feed Mark A] + (WT - [Point B]) / 4 if [Point C] >= WT > [Point B]
        F23 = [Inner Feed Mark C] if [Point D] >= WT > [Point C]
        F23 = [Inner Feed Mark C] + (WT - [Point D]) * ([Inner Feed Mark E]/([Outer Feed Mark E] * 2 + [Inner Feed Mark E] * 2)) if [Point E] >= WT > [Point D]
        F23 = [Inner Feed Mark E] if [Point H] >= WT > [Point E]
        F23 = [Inner Feed Mark E] + (WT - [Point H]) / 10 if WT > [Point H]

        Outer Tanks
        O = 0 if WT < [Point A]
        O = (WT - [Point A]) / 2 if [Point B] >= WT >= [Point A]
        O = [Outer Tank Mark B] if [Point G] >= WT > [Point B]
        O = [Outer Tank Mark B] + (WT - [Point G]) / 2 if [Point H] >= WT > [Point G]
        O = [Outer Tank Mark H] + (WT - [Point H]) / 10 if WT > [Point H]

        Mid Tanks
        M = 0 if WT < [Point E]
        M = (WT - [Point E]) / 2 if [Point F] >= WT >= [Point E]
        M = [Mid Tank Mark F] if [Point H] >= WT > [Point F]
        M = [Mid Tank Mark F] + (WT - [Point H]) / 10 if WT > [Point H]

        Inner Tanks
        I = 0 if WT < [Point C]
        I = (WT - [Point C]) / 2 if [Point D] >= WT >=  [Point C]
        I = [Inner Tank Mark D] if [Point F] >= WT > [Point D]
        I = [Inner Tank Mark D] + (WT - [Point F]) / 2 if [Point G] >= WT > [Point F]
        I = [Inner Tank Mark G] if [Point H] >= WT > [Point G]
        I = [Inner Tank Mark G] + (WT - [Point H]) / 10 if WT > [Point H]
        */
    }, []);

    const updateDesiredFuel = (desiredFuelKg: string) => {
        let fuelWeightKg = 0;
        if (desiredFuelKg.length > 0) {
            fuelWeightKg = parseInt(desiredFuelKg);
            if (fuelWeightKg > TOTAL_MAX_FUEL_KG) {
                fuelWeightKg = round(TOTAL_MAX_FUEL_KG);
            }
            // setInputValue(fuelWeightKg);
        }
        // TODO: Remove
        setFuelDesiredPercent((fuelWeightKg / TOTAL_MAX_FUEL_KG) * 100);
        calculateDesiredFuelKg(fuelWeightKg);
    };

    // TODO: Remove
    const updateDesiredFuelPercent = (percent: number) => {
        if (percent < 0.5) {
            percent = 0;
        }
        setFuelDesiredPercent(percent);
        const fuel = Math.round(TOTAL_MAX_FUEL_KG * (percent / 100));
        updateDesiredFuel(fuel.toString());
    };

    /*
    const setDesiredFuel = (fuel: number) => {
        fuel -= (OUTER_CELL_GALLONS) * 2;
        const outerTank = (((OUTER_CELL_GALLONS) * 2) + Math.min(fuel, 0)) / 2;
        setLOutTarget(outerTank);
        setROutTarget(outerTank);
        if (fuel <= 0) {
            setLInnTarget(0);
            setRInnTarget(0);
            setCenterTarget(0);
            return;
        }
        fuel -= (INNER_CELL_GALLONS) * 2;
        const innerTank = (((INNER_CELL_GALLONS) * 2) + Math.min(fuel, 0)) / 2;
        setLInnTarget(innerTank);
        setRInnTarget(innerTank);
        if (fuel <= 0) {
            setCenterTarget(0);
            return;
        }
        setCenterTarget(fuel);
    };
    */

    /*
    const updateDesiredFuel = (value: string) => {
        let fuel = 0;
        let originalFuel = 0;
        if (value.length > 0) {
            originalFuel = parseInt(value);
            fuel = convertToGallon(originalFuel);
            if (originalFuel > totalFuel()) {
                originalFuel = round(totalFuel());
            }
            setInputValue(originalFuel);
        }
        if (fuel > TOTAL_FUEL_GALLONS) {
            fuel = TOTAL_FUEL_GALLONS + 2;
        }
        setTotalTarget(fuel);
        setSliderValue((fuel / TOTAL_FUEL_GALLONS) * 100);
        setDesiredFuel(fuel);
    };

    const updateSlider = (value: number) => {
        if (value < 2) {
            value = 0;
        }
        setSliderValue(value);
        const fuel = Math.round(totalFuel() * (value / 100));
        updateDesiredFuel(fuel.toString());
    };

    const calculateEta = () => {
        if (round(totalTarget) === totalCurrentGallon() || refuelRate === '2') { // instant
            return ' 0';
        }
        let estimatedTimeSeconds = 0;
        const totalWingFuel = TOTAL_FUEL_GALLONS - CENTER_TANK_GALLONS;
        const differentialFuelWings = Math.abs(currentWingFuel() - targetWingFuel());
        const differentialFuelCenter = Math.abs(centerTarget - centerCurrent);
        estimatedTimeSeconds += (differentialFuelWings / totalWingFuel) * wingTotalRefuelTimeSeconds;
        estimatedTimeSeconds += (differentialFuelCenter / CENTER_TANK_GALLONS) * CenterTotalRefuelTimeSeconds;
        if (refuelRate === '1') { // fast
            estimatedTimeSeconds /= 5;
        }
        if (estimatedTimeSeconds < 35) {
            return ' 0.5';
        }
        return ` ${Math.round(estimatedTimeSeconds / 60)}`;
    };
     */

    /*
    const switchRefuelState = () => {
        if (airplaneCanRefuel()) {
            setRefuelStartedByUser(!refuelStartedByUser);
        }
    };
    */

    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    const handleSimbriefFuelSync = () => {
        let fuelToLoad = -1;

        if (Units.usingMetric) {
            if (simbriefUnits === 'kgs') {
                fuelToLoad = roundUpNearest100(simbriefPlanRamp);
            } else {
                fuelToLoad = roundUpNearest100(Units.poundToKilogram(simbriefPlanRamp));
            }
        } else if (simbriefUnits === 'kgs') {
            fuelToLoad = roundUpNearest100(Units.kilogramToPound(simbriefPlanRamp));
        } else {
            fuelToLoad = roundUpNearest100(simbriefPlanRamp);
        }

        updateDesiredFuel(fuelToLoad.toString());
    };

    const roundUpNearest100 = (plannedFuel: number) => Math.ceil(plannedFuel / 100) * 100;

    useEffect(() => {
        setFuelDesiredPercent((totalFuelWeightKg / TOTAL_MAX_FUEL_KG) * 100);
    }, [totalFuelWeightKg]);

    return (
        <div className="flex relative flex-col justify-center">
            <div className="flex relative flex-row justify-between w-full h-content-section-reduced">
                <Card className="flex absolute top-6 left-0 w-fit" childrenContainerClassName={`w-full ${simbriefDataLoaded ? 'rounded-r-none' : ''}`}>
                    <table className="table-fixed">
                        <tbody>
                            <tr>
                                <td className="px-2 font-light whitespace-nowrap text-md">
                                    Feed One
                                </td>
                                <td className="px-2 font-light whitespace-nowrap text-md">
                                    <ProgressBar
                                        height="10px"
                                        width="80px"
                                        displayBar={false}
                                        completedBarBegin={100}
                                        isLabelVisible={false}
                                        bgcolor="var(--color-highlight)"
                                        completed={feedOneGal * FUEL_GALLONS_TO_KG / OUTER_FEED_MAX_KG * 100}
                                    />
                                </td>
                                <td className="px-2 my-2 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(feedOneGal * FUEL_GALLONS_TO_KG)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                            <tr>
                                <td className="px-2 font-light whitespace-nowrap text-md">
                                    Feed Two
                                </td>
                                <td className="px-2 font-light whitespace-nowrap text-md">
                                    <ProgressBar
                                        height="10px"
                                        width="80px"
                                        displayBar={false}
                                        completedBarBegin={100}
                                        isLabelVisible={false}
                                        bgcolor="var(--color-highlight)"
                                        completed={feedTwoGal * FUEL_GALLONS_TO_KG / INNER_FEED_MAX_KG * 100}
                                    />
                                </td>
                                <td className="px-2 my-2 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(feedTwoGal * FUEL_GALLONS_TO_KG)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                            <tr>
                                <td className="px-2 font-light whitespace-nowrap text-md">
                                    Left Inner
                                </td>
                                <td className="px-2 font-light whitespace-nowrap text-md">
                                    <ProgressBar
                                        height="10px"
                                        width="80px"
                                        displayBar={false}
                                        completedBarBegin={100}
                                        isLabelVisible={false}
                                        bgcolor="var(--color-highlight)"
                                        completed={leftInnerGal * FUEL_GALLONS_TO_KG / INNER_TANK_MAX_KG * 100}
                                    />
                                </td>
                                <td className="px-2 my-2 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(leftInnerGal * FUEL_GALLONS_TO_KG)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                            <tr>
                                <td className="px-2 font-light whitespace-nowrap text-md">
                                    Left Mid
                                </td>
                                <td className="px-2 font-light whitespace-nowrap text-md">
                                    <ProgressBar
                                        height="10px"
                                        width="80px"
                                        displayBar={false}
                                        completedBarBegin={100}
                                        isLabelVisible={false}
                                        bgcolor="var(--color-highlight)"
                                        completed={leftMidGal * FUEL_GALLONS_TO_KG / MID_TANK_MAX_KG * 100}
                                    />
                                </td>
                                <td className="px-2 my-2 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(leftMidGal * FUEL_GALLONS_TO_KG)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                            <tr>
                                <td className="px-2 font-light whitespace-nowrap text-md">
                                    Left Outer
                                </td>
                                <td className="px-2 font-light whitespace-nowrap text-md">
                                    <ProgressBar
                                        height="10px"
                                        width="80px"
                                        displayBar={false}
                                        completedBarBegin={100}
                                        isLabelVisible={false}
                                        bgcolor="var(--color-highlight)"
                                        completed={leftOuterGal * FUEL_GALLONS_TO_KG / OUTER_TANK_MAX_KG * 100}
                                    />
                                </td>
                                <td className="px-2 my-2 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(leftOuterGal * FUEL_GALLONS_TO_KG)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                        </tbody>
                    </table>
                </Card>
                <Card className="flex absolute top-6 right-0 w-fit" childrenContainerClassName={`w-full ${simbriefDataLoaded ? 'rounded-r-none' : ''}`}>
                    <table className="table-fixed">
                        <tbody>
                            <tr>
                                <td className="px-2 font-light whitespace-nowrap text-md">
                                    Feed Three
                                </td>
                                <td className="px-2 font-light whitespace-nowrap text-md">
                                    <ProgressBar
                                        height="10px"
                                        width="80px"
                                        displayBar={false}
                                        completedBarBegin={100}
                                        isLabelVisible={false}
                                        bgcolor="var(--color-highlight)"
                                        completed={feedThreeGal * FUEL_GALLONS_TO_KG / INNER_FEED_MAX_KG * 100}
                                    />
                                </td>
                                <td className="px-2 my-2 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(feedThreeGal * FUEL_GALLONS_TO_KG)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                            <tr>
                                <td className="px-2 font-light whitespace-nowrap text-md">
                                    Feed Four
                                </td>
                                <td className="px-2 font-light whitespace-nowrap text-md">
                                    <ProgressBar
                                        height="10px"
                                        width="80px"
                                        displayBar={false}
                                        completedBarBegin={100}
                                        isLabelVisible={false}
                                        bgcolor="var(--color-highlight)"
                                        completed={feedFourGal * FUEL_GALLONS_TO_KG / OUTER_FEED_MAX_KG * 100}
                                    />
                                </td>
                                <td className="px-2 my-2 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(feedFourGal * FUEL_GALLONS_TO_KG)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                            <tr>
                                <td className="px-2 font-light whitespace-nowrap text-md">
                                    Right Inner
                                </td>
                                <td className="px-2 font-light whitespace-nowrap text-md">
                                    <ProgressBar
                                        height="10px"
                                        width="80px"
                                        displayBar={false}
                                        completedBarBegin={100}
                                        isLabelVisible={false}
                                        bgcolor="var(--color-highlight)"
                                        completed={rightInnerGal * FUEL_GALLONS_TO_KG / INNER_TANK_MAX_KG * 100}
                                    />
                                </td>
                                <td className="px-2 my-2 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(rightInnerGal * FUEL_GALLONS_TO_KG)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                            <tr>
                                <td className="px-2 font-light whitespace-nowrap text-md">
                                    Right Mid
                                </td>
                                <td className="px-2 font-light whitespace-nowrap text-md">
                                    <ProgressBar
                                        height="10px"
                                        width="80px"
                                        displayBar={false}
                                        completedBarBegin={100}
                                        isLabelVisible={false}
                                        bgcolor="var(--color-highlight)"
                                        completed={rightMidGal * FUEL_GALLONS_TO_KG / MID_TANK_MAX_KG * 100}
                                    />
                                </td>
                                <td className="px-2 my-2 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(rightMidGal * FUEL_GALLONS_TO_KG)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                            <tr>
                                <td className="px-2 font-light whitespace-nowrap text-md">
                                    Right Outer
                                </td>
                                <td className="px-2 font-light whitespace-nowrap text-md">
                                    <ProgressBar
                                        height="10px"
                                        width="80px"
                                        displayBar={false}
                                        completedBarBegin={100}
                                        isLabelVisible={false}
                                        bgcolor="var(--color-highlight)"
                                        completed={rightOuterGal * FUEL_GALLONS_TO_KG / OUTER_TANK_MAX_KG * 100}
                                    />
                                </td>
                                <td className="px-2 my-2 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(rightOuterGal * FUEL_GALLONS_TO_KG)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                        </tbody>
                    </table>
                </Card>
                <A380FuelOutline
                    className="flex absolute inset-x-0 top-20 right-4 mx-auto w-full h-full text-theme-text"
                    feed1Percent={(Math.max(feedThreeGal * FUEL_GALLONS_TO_KG, 0) / OUTER_FEED_MAX_KG) * 100}
                    feed2Percent={(Math.max(feedThreeGal * FUEL_GALLONS_TO_KG, 0) / INNER_FEED_MAX_KG) * 100}
                    feed3Percent={(Math.max(feedThreeGal * FUEL_GALLONS_TO_KG, 0) / INNER_FEED_MAX_KG) * 100}
                    feed4Percent={(Math.max(feedThreeGal * FUEL_GALLONS_TO_KG, 0) / OUTER_FEED_MAX_KG) * 100}
                    leftInnerPercent={(Math.max(leftInnerGal * FUEL_GALLONS_TO_KG, 0) / INNER_TANK_MAX_KG) * 100}
                    leftMidPercent={(Math.max(leftMidGal * FUEL_GALLONS_TO_KG, 0) / MID_TANK_MAX_KG) * 100}
                    leftOuterPercent={(Math.max(leftOuterGal * FUEL_GALLONS_TO_KG, 0) / OUTER_TANK_MAX_KG) * 100}
                    rightInnerPercent={(Math.max(rightInnerGal * FUEL_GALLONS_TO_KG, 0) / INNER_TANK_MAX_KG) * 100}
                    rightMidPercent={(Math.max(rightMidGal * FUEL_GALLONS_TO_KG, 0) / MID_TANK_MAX_KG) * 100}
                    rightOuterPercent={(Math.max(rightOuterGal * FUEL_GALLONS_TO_KG, 0) / OUTER_TANK_MAX_KG) * 100}
                    trimPercent={(Math.max(trimGal * FUEL_GALLONS_TO_KG, 0) / TRIM_TANK_MAX_KG) * 100}
                />
            </div>

            <Card className="flex absolute bottom-40 left-20" childrenContainerClassName={`w-full ${simbriefDataLoaded ? 'rounded-r-none' : ''}`}>
                <table className="table-fixed">
                    <tbody>
                        <tr>
                            <td className="px-2 font-light whitespace-nowrap text-md">
                                Trim
                            </td>
                            <td className="px-2 font-light whitespace-nowrap text-md">
                                <ProgressBar
                                    height="10px"
                                    width="80px"
                                    displayBar={false}
                                    completedBarBegin={100}
                                    isLabelVisible={false}
                                    bgcolor="var(--color-highlight)"
                                    completed={trimGal * FUEL_GALLONS_TO_KG / TRIM_TANK_MAX_KG * 100}
                                />
                            </td>
                            <td className="px-2 my-2 font-mono font-light whitespace-nowrap text-md">
                                <ValueInput
                                    min={0}
                                    max={Math.ceil(Units.kilogramToUser(TRIM_TANK_MAX_KG))}
                                    value={Units.kilogramToUser(trimGal * FUEL_GALLONS_TO_KG)}
                                    onBlur={(x) => {
                                        if (!Number.isNaN(parseInt(x) || parseInt(x) === 0)) {
                                            setTrim(Units.userToKilogram(parseInt(x)) / FUEL_GALLONS_TO_KG);
                                            // TODO: Remove placeholder refueling setting
                                            setRefuelStartedByUser(true);
                                            setRefuelRate('2');
                                        }
                                    }}
                                    unit={massUnitForDisplay}
                                />
                            </td>
                        </tr>
                    </tbody>
                </table>
            </Card>

            <div className="flex overflow-x-hidden absolute bottom-0 left-0 z-10 flex-row max-w-3xl rounded-2xl border border-theme-accentborder-2">
                <div className="py-3 px-5 space-y-4">
                    <div className="flex flex-row justify-between items-center">
                        <div className="flex flex-row items-center space-x-3">
                            <h2 className="font-medium">{t('Ground.Fuel.Refuel')}</h2>
                            <p className="text-theme-accent" />
                        </div>
                        <p>{`${t('Ground.Fuel.EstimatedDuration')}: 0`}</p>
                    </div>
                    <div className="flex flex-row items-center space-x-6">
                        <Slider
                            style={{ width: '28rem' }}
                            value={fuelDesiredPercent}
                            onChange={updateDesiredFuelPercent}
                        />
                        <div className="flex flex-row">
                            <ValueSimbriefInput
                                min={0}
                                max={Math.ceil(Units.kilogramToUser(TOTAL_MAX_FUEL_KG))}
                                value={Units.kilogramToUser(totalFuelWeightKg)}
                                onBlur={(x) => {
                                    if (!Number.isNaN(parseInt(x) || parseInt(x) === 0)) {
                                        calculateDesiredFuelKg(Units.userToKilogram(parseInt(x)));
                                        // TODO: Remove placeholder refueling setting
                                        setRefuelStartedByUser(true);
                                        setRefuelRate('2');
                                    }
                                }}
                                unit={massUnitForDisplay}
                                showSimbriefButton={showSimbriefButton}
                                onClickSync={handleSimbriefFuelSync}
                                disabled={gsxFuelSyncEnabled === 1}
                            />
                        </div>
                    </div>
                </div>
                {/*
                    <div
                        className="flex justify-center items-center w-20 bg-current text-theme-accent"
                        onClick={() => null switchRefuelState()}
                    >
                        <div className={`${airplaneCanRefuel() ? 'text-white' : 'text-theme-unselected'}`}>
                            <PlayFill size={50} className={refuelStartedByUser ? 'hidden' : ''} />
                            <StopCircleFill size={50} className={refuelStartedByUser ? '' : 'hidden'} />
                        </div>
                    </div>
                */}
            </div>

            <div className="flex overflow-x-hidden absolute right-6 bottom-0 flex-col justify-center items-center py-3 px-6 space-y-2 rounded-2xl border border-theme-accent">
                <h2 className="flex font-medium">{t('Ground.Fuel.RefuelTime')}</h2>
                <SelectGroup>
                    <SelectItem selected={airplaneCanRefuel() ? refuelRate === '2' : !airplaneCanRefuel()} onSelect={() => setRefuelRate('2')}>{t('Settings.Instant')}</SelectItem>

                    <TooltipWrapper text={`${!airplaneCanRefuel() && t('Ground.Fuel.TT.AircraftMustBeColdAndDarkToChangeRefuelTimes')}`}>
                        <div>
                            <SelectItem className={`${!airplaneCanRefuel() && 'opacity-20'}`} disabled={!airplaneCanRefuel()} selected={refuelRate === '1'} onSelect={() => setRefuelRate('1')}>{t('Settings.Fast')}</SelectItem>
                        </div>
                    </TooltipWrapper>

                    <TooltipWrapper text={`${!airplaneCanRefuel() && t('Ground.Fuel.TT.AircraftMustBeColdAndDarkToChangeRefuelTimes')}`}>
                        <div>
                            <SelectItem className={`${!airplaneCanRefuel() && 'opacity-20'}`} disabled={!airplaneCanRefuel()} selected={refuelRate === '0'} onSelect={() => setRefuelRate('0')}>{t('Settings.Real')}</SelectItem>
                        </div>
                    </TooltipWrapper>
                </SelectGroup>
            </div>
        </div>
    );
};
