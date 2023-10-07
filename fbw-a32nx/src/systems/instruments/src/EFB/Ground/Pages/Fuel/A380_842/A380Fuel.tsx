/* eslint-disable max-len */
import React, { useCallback, useState } from 'react';
import { round } from 'lodash';
// import { CloudArrowDown, PlayFill, StopCircleFill } from 'react-bootstrap-icons';
import { useSimVar, usePersistentNumberProperty, usePersistentProperty, Units } from '@flybywiresim/fbw-sdk';
// import Slider from 'rc-slider';
import Card from 'instruments/src/EFB/UtilComponents/Card/Card';
// import { FuelInputTable } from '../FuelInputTable';
import { t } from '../../../../translation';
// import { TooltipWrapper } from '../../../../UtilComponents/TooltipWrapper';
// import { SelectGroup, SelectItem } from '../../../../UtilComponents/Form/Select';
// import { ProgressBar } from '../../../../UtilComponents/Progress/Progress';
import { SimpleInput } from '../../../../UtilComponents/Form/SimpleInput/SimpleInput';
// import { OverWingOutline } from '../../../../Assets/OverWingOutline';

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
    <div className="relative w-44">
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
            <span className="flex justify-end pr-2 w-20 text-2xl">
                <span className="text-2xl text-gray-400">{'0'.repeat(leadingZeroCount)}</span>
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
    const [TOTAL_FUEL_KG] = useState(TOTAL_FUEL_GALLONS * FUEL_GALLONS_TO_KG);

    const [eng1Running] = useSimVar('ENG COMBUSTION:1', 'Bool', 1_000);
    const [eng4Running] = useSimVar('ENG COMBUSTION:4', 'Bool', 1_000);
    const [refuelRate, setRefuelRate] = usePersistentProperty('REFUEL_RATE_SETTING');

    const [INNER_FEED_MAX_KG] = useState(7753.2 * FUEL_GALLONS_TO_KG);
    const [OUTER_FEED_MAX_KG] = useState(7299.6 * FUEL_GALLONS_TO_KG);
    const [INNER_TANK_MAX_KG] = useState(12189.4 * FUEL_GALLONS_TO_KG);
    const [MID_TANK_MAX_KG] = useState(9632 * FUEL_GALLONS_TO_KG);
    const [OUTER_TANK_MAX_KG] = useState(2731.5 * FUEL_GALLONS_TO_KG);
    const [TRIM_TANK_MAX_KG] = useState(6260.3 * FUEL_GALLONS_TO_KG);

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
    const [totalFuelWeightKg] = useSimVar('FUEL TOTAL QUANTITY WEIGHT', 'Kilograms', 2_000); // 6260.3

    // TODO: Remove debug override
    const [refuelStartedByUser, setRefuelStartedByUser] = useSimVar('L:A32NX_REFUEL_STARTED_BY_USR', 'Bool');

    // GSX
    const [gsxFuelSyncEnabled] = usePersistentNumberProperty('GSX_FUEL_SYNC', 0);
    const [gsxFuelHoseConnected] = useSimVar('L:FSDT_GSX_FUELHOSE_CONNECTED', 'Number');

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

    const setDesiredFuelWeight = useCallback((fuelWeightKg: number) => {
        let fuelWeightRemaining = fuelWeightKg;
        fuelWeightRemaining -= OUTER_FEED_MAX_KG * 4;
        const feed = Math.max(((OUTER_FEED_MAX_KG) + Math.min(fuelWeightRemaining, 0) / 4), 0) / FUEL_GALLONS_TO_KG;

        setFeedOne(feed);
        setFeedTwo(feed);
        setFeedThree(feed);
        setFeedFour(feed);

        const deltaFeed = (INNER_FEED_MAX_KG - OUTER_FEED_MAX_KG);
        fuelWeightRemaining -= deltaFeed * 2;

        const innerFeed = Math.max((deltaFeed + Math.min(fuelWeightRemaining, 0) / 2), 0) / FUEL_GALLONS_TO_KG;
        setFeedTwo(feed + innerFeed);
        setFeedThree(feed + innerFeed);

        fuelWeightRemaining -= INNER_TANK_MAX_KG * 2;

        const innerTank = Math.max(((INNER_TANK_MAX_KG) + Math.min(fuelWeightRemaining, 0) / 2), 0) / FUEL_GALLONS_TO_KG;
        setLeftInner(innerTank);
        setRightInner(innerTank);

        fuelWeightRemaining -= MID_TANK_MAX_KG * 2;

        const midTank = Math.max(((MID_TANK_MAX_KG) + Math.min(fuelWeightRemaining, 0) / 2), 0) / FUEL_GALLONS_TO_KG;
        setLeftMid(midTank);
        setRightMid(midTank);

        fuelWeightRemaining -= OUTER_TANK_MAX_KG * 2;

        const outerTank = Math.max(((OUTER_TANK_MAX_KG) + Math.min(fuelWeightRemaining, 0) / 2), 0) / FUEL_GALLONS_TO_KG;
        setLeftOuter(outerTank);
        setRightOuter(outerTank);

        setTrim(Math.min(Math.max(fuelWeightRemaining / FUEL_GALLONS_TO_KG, 0), TRIM_TANK_MAX_KG));
    }, []);

    const updateDesiredFuel = (desiredFuelKg: string) => {
        let fuelWeightKg = 0;
        if (desiredFuelKg.length > 0) {
            fuelWeightKg = parseInt(desiredFuelKg);
            if (fuelWeightKg > TOTAL_FUEL_KG) {
                fuelWeightKg = round(TOTAL_FUEL_KG);
            }
            // setInputValue(fuelWeightKg);
        }
        setDesiredFuelWeight(fuelWeightKg);
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
    const handleFuelAutoFill = () => {
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

    return (
        <div className="flex flex-row justify-between px-4">
            <div className="flex flex-row w-full">
                <Card className="w-full col-1" childrenContainerClassName={`w-full ${simbriefDataLoaded ? 'rounded-r-none' : ''}`}>
                    <table className="w-full table-fixed">
                        <thead className="px-8 mx-2 w-full border-b">
                            <tr className="py-2">
                                <th scope="col" className="py-2 px-4 w-2/5 font-medium text-left text-md">
                                    {'!!! TEMPORARY WIP !!! '}
                                </th>
                                <th scope="col" className="py-2 px-4 w-1/4 font-medium text-left text-md">
                                    {t('Ground.Payload.Planned')}
                                </th>
                                <th scope="col" className="py-2 px-4 w-1/4 font-medium text-left text-md">
                                    {t('Ground.Payload.Current')}
                                </th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr className="h-2" />
                            <tr>
                                <td className="px-4 font-light whitespace-nowrap text-md">
                                    Feed One
                                </td>
                                <td className="mx-8">
                                    {/* <TooltipWrapper text={`${t('Ground.Payload.TT.MaxPassengers')} ${maxPax}`}> */}
                                    <div className={`px-4 font-light whitespace-nowrap text-md ${/* (gsxPayloadSyncEnabled && boardingStarted) */ false ? 'pointer-events-none' : ''}`}>
                                        <ValueInput
                                            min={0}
                                            max={Math.ceil(Units.kilogramToUser(OUTER_FEED_MAX_KG))}
                                            value={Units.kilogramToUser(feedOneGal * FUEL_GALLONS_TO_KG)}
                                            onBlur={(x) => {
                                                if (!Number.isNaN(parseInt(x) || parseInt(x) === 0)) {
                                                    setFeedOne(Units.userToKilogram(parseInt(x)) / FUEL_GALLONS_TO_KG);
                                                    // TODO: Remove placeholder refueling setting
                                                    // TODO: Remove placeholder refueling setting
                                                    setRefuelStartedByUser(true);
                                                    setRefuelRate('2');
                                                }
                                            }}
                                            unit={massUnitForDisplay}
                                            disabled={gsxFuelSyncEnabled === 1}
                                        />
                                    </div>
                                    {/*  </TooltipWrapper> */}
                                </td>
                                <td className="px-4 w-20 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(feedOneGal * FUEL_GALLONS_TO_KG)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                            <tr>
                                <td className="px-4 font-light whitespace-nowrap text-md">
                                    Feed Two
                                </td>
                                <td className="mx-8">
                                    {/* <TooltipWrapper text={`${t('Ground.Payload.TT.MaxPassengers')} ${maxPax}`}> */}
                                    <div className={`px-4 font-light whitespace-nowrap text-md ${/* (gsxPayloadSyncEnabled && boardingStarted) */ false ? 'pointer-events-none' : ''}`}>
                                        <ValueInput
                                            min={0}
                                            max={Math.ceil(Units.kilogramToUser(INNER_FEED_MAX_KG))}
                                            value={Units.kilogramToUser(feedTwoGal * FUEL_GALLONS_TO_KG)}
                                            onBlur={(x) => {
                                                if (!Number.isNaN(parseInt(x) || parseInt(x) === 0)) {
                                                    setFeedTwo(Units.userToKilogram(parseInt(x)) / FUEL_GALLONS_TO_KG);
                                                    // TODO: Remove placeholder refueling setting
                                                    // TODO: Remove placeholder refueling setting
                                                    setRefuelStartedByUser(true);
                                                    setRefuelRate('2');
                                                }
                                            }}
                                            unit={massUnitForDisplay}
                                            disabled={gsxFuelSyncEnabled === 1}
                                        />
                                    </div>
                                    {/*  </TooltipWrapper> */}
                                </td>
                                <td className="px-4 w-20 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(feedTwoGal * FUEL_GALLONS_TO_KG)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                            <tr>
                                <td className="px-4 font-light whitespace-nowrap text-md">
                                    Feed Three
                                </td>
                                <td className="mx-8">
                                    {/* <TooltipWrapper text={`${t('Ground.Payload.TT.MaxPassengers')} ${maxPax}`}> */}
                                    <div className={`px-4 font-light whitespace-nowrap text-md ${/* (gsxPayloadSyncEnabled && boardingStarted) */ false ? 'pointer-events-none' : ''}`}>
                                        <ValueInput
                                            min={0}
                                            max={Math.ceil(Units.kilogramToUser(INNER_FEED_MAX_KG))}
                                            value={Units.kilogramToUser(feedThreeGal * FUEL_GALLONS_TO_KG)}
                                            onBlur={(x) => {
                                                if (!Number.isNaN(parseInt(x) || parseInt(x) === 0)) {
                                                    setFeedThree(Units.userToKilogram(parseInt(x)) / FUEL_GALLONS_TO_KG);
                                                    // TODO: Remove placeholder refueling setting
                                                    // TODO: Remove placeholder refueling setting
                                                    setRefuelStartedByUser(true);
                                                    setRefuelRate('2');
                                                }
                                            }}
                                            unit={massUnitForDisplay}
                                            disabled={gsxFuelSyncEnabled === 1}
                                        />
                                    </div>
                                    {/*  </TooltipWrapper> */}
                                </td>
                                <td className="px-4 w-20 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(feedThreeGal * FUEL_GALLONS_TO_KG)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                            <tr>
                                <td className="px-4 font-light whitespace-nowrap text-md">
                                    Feed Four
                                </td>
                                <td className="mx-8">
                                    {/* <TooltipWrapper text={`${t('Ground.Payload.TT.MaxPassengers')} ${maxPax}`}> */}
                                    <div className={`px-4 font-light whitespace-nowrap text-md ${/* (gsxPayloadSyncEnabled && boardingStarted) */ false ? 'pointer-events-none' : ''}`}>
                                        <ValueInput
                                            min={0}
                                            max={Math.ceil(Units.kilogramToUser(OUTER_FEED_MAX_KG))}
                                            value={Units.kilogramToUser(feedFourGal * FUEL_GALLONS_TO_KG)}
                                            onBlur={(x) => {
                                                if (!Number.isNaN(parseInt(x) || parseInt(x) === 0)) {
                                                    setFeedFour(Units.userToKilogram(parseInt(x)) / FUEL_GALLONS_TO_KG);
                                                    // TODO: Remove placeholder refueling setting
                                                    // TODO: Remove placeholder refueling setting
                                                    setRefuelStartedByUser(true);
                                                    setRefuelRate('2');
                                                }
                                            }}
                                            unit={massUnitForDisplay}
                                            disabled={gsxFuelSyncEnabled === 1}
                                        />
                                    </div>
                                    {/*  </TooltipWrapper> */}
                                </td>
                                <td className="px-4 w-20 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(feedFourGal * FUEL_GALLONS_TO_KG)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                            <tr>
                                <td className="px-4 font-light whitespace-nowrap text-md">
                                    Left Inner
                                </td>
                                <td className="mx-8">
                                    {/* <TooltipWrapper text={`${t('Ground.Payload.TT.MaxPassengers')} ${maxPax}`}> */}
                                    <div className={`px-4 font-light whitespace-nowrap text-md ${/* (gsxPayloadSyncEnabled && boardingStarted) */ false ? 'pointer-events-none' : ''}`}>
                                        <ValueInput
                                            min={0}
                                            max={Math.ceil(Units.kilogramToUser(INNER_TANK_MAX_KG))}
                                            value={Units.kilogramToUser(leftInnerGal * FUEL_GALLONS_TO_KG)}
                                            onBlur={(x) => {
                                                if (!Number.isNaN(parseInt(x) || parseInt(x) === 0)) {
                                                    setLeftInner(Units.userToKilogram(parseInt(x)) / FUEL_GALLONS_TO_KG);
                                                    // TODO: Remove placeholder refueling setting
                                                    setRefuelStartedByUser(true);
                                                    setRefuelRate('2');
                                                }
                                            }}
                                            unit={massUnitForDisplay}
                                            disabled={gsxFuelSyncEnabled === 1}
                                        />
                                    </div>
                                    {/*  </TooltipWrapper> */}
                                </td>
                                <td className="px-4 w-20 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(leftInnerGal * FUEL_GALLONS_TO_KG)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                            <tr>
                                <td className="px-4 font-light whitespace-nowrap text-md">
                                    Right Inner
                                </td>
                                <td className="mx-8">
                                    {/* <TooltipWrapper text={`${t('Ground.Payload.TT.MaxPassengers')} ${maxPax}`}> */}
                                    <div className={`px-4 font-light whitespace-nowrap text-md ${/* (gsxPayloadSyncEnabled && boardingStarted) */ false ? 'pointer-events-none' : ''}`}>
                                        <ValueInput
                                            min={0}
                                            max={Math.ceil(Units.kilogramToUser(INNER_TANK_MAX_KG))}
                                            value={Units.kilogramToUser(rightInnerGal * FUEL_GALLONS_TO_KG)}
                                            onBlur={(x) => {
                                                if (!Number.isNaN(parseInt(x) || parseInt(x) === 0)) {
                                                    setRightInner(Units.userToKilogram(parseInt(x)) / FUEL_GALLONS_TO_KG);
                                                    // TODO: Remove placeholder refueling setting
                                                    setRefuelStartedByUser(true);
                                                    setRefuelRate('2');
                                                }
                                            }}
                                            unit={massUnitForDisplay}
                                            disabled={gsxFuelSyncEnabled === 1}
                                        />
                                    </div>
                                    {/*  </TooltipWrapper> */}
                                </td>
                                <td className="px-4 w-20 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(rightInnerGal * FUEL_GALLONS_TO_KG)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                            <tr>
                                <td className="px-4 font-light whitespace-nowrap text-md">
                                    Left Mid
                                </td>
                                <td className="mx-8">
                                    {/* <TooltipWrapper text={`${t('Ground.Payload.TT.MaxPassengers')} ${maxPax}`}> */}
                                    <div className={`px-4 font-light whitespace-nowrap text-md ${/* (gsxPayloadSyncEnabled && boardingStarted) */ false ? 'pointer-events-none' : ''}`}>
                                        <ValueInput
                                            min={0}
                                            max={Math.ceil(Units.kilogramToUser(MID_TANK_MAX_KG))}
                                            value={Units.kilogramToUser(leftMidGal * FUEL_GALLONS_TO_KG)}
                                            onBlur={(x) => {
                                                if (!Number.isNaN(parseInt(x) || parseInt(x) === 0)) {
                                                    setLeftMid(Units.userToKilogram(parseInt(x)) / FUEL_GALLONS_TO_KG);
                                                    // TODO: Remove placeholder refueling setting
                                                    setRefuelStartedByUser(true);
                                                    setRefuelRate('2');
                                                }
                                            }}
                                            unit={massUnitForDisplay}
                                            disabled={gsxFuelSyncEnabled === 1}
                                        />
                                    </div>
                                    {/*  </TooltipWrapper> */}
                                </td>
                                <td className="px-4 w-20 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(leftMidGal * FUEL_GALLONS_TO_KG)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                            <tr>
                                <td className="px-4 font-light whitespace-nowrap text-md">
                                    Right Mid
                                </td>
                                <td className="mx-8">
                                    {/* <TooltipWrapper text={`${t('Ground.Payload.TT.MaxPassengers')} ${maxPax}`}> */}
                                    <div className={`px-4 font-light whitespace-nowrap text-md ${/* (gsxPayloadSyncEnabled && boardingStarted) */ false ? 'pointer-events-none' : ''}`}>
                                        <ValueInput
                                            min={0}
                                            max={Math.ceil(Units.kilogramToUser(MID_TANK_MAX_KG))}
                                            value={Units.kilogramToUser(rightMidGal * FUEL_GALLONS_TO_KG)}
                                            onBlur={(x) => {
                                                if (!Number.isNaN(parseInt(x) || parseInt(x) === 0)) {
                                                    setRightMid(Units.userToKilogram(parseInt(x)) / FUEL_GALLONS_TO_KG);
                                                    // TODO: Remove placeholder refueling setting
                                                    setRefuelStartedByUser(true);
                                                    setRefuelRate('2');
                                                }
                                            }}
                                            unit={massUnitForDisplay}
                                            disabled={gsxFuelSyncEnabled === 1}
                                        />
                                    </div>
                                    {/*  </TooltipWrapper> */}
                                </td>
                                <td className="px-4 w-20 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(rightMidGal * FUEL_GALLONS_TO_KG)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                            <tr>
                                <td className="px-4 font-light whitespace-nowrap text-md">
                                    Left Outer
                                </td>
                                <td className="mx-8">
                                    {/* <TooltipWrapper text={`${t('Ground.Payload.TT.MaxPassengers')} ${maxPax}`}> */}
                                    <div className={`px-4 font-light whitespace-nowrap text-md ${/* (gsxPayloadSyncEnabled && boardingStarted) */ false ? 'pointer-events-none' : ''}`}>
                                        <ValueInput
                                            min={0}
                                            max={Math.ceil(Units.kilogramToUser(OUTER_TANK_MAX_KG))}
                                            value={Units.kilogramToUser(leftOuterGal * FUEL_GALLONS_TO_KG)}
                                            onBlur={(x) => {
                                                if (!Number.isNaN(parseInt(x) || parseInt(x) === 0)) {
                                                    setLeftOuter(Units.userToKilogram(parseInt(x)) / FUEL_GALLONS_TO_KG);
                                                    // TODO: Remove placeholder refueling setting
                                                    setRefuelStartedByUser(true);
                                                    setRefuelRate('2');
                                                }
                                            }}
                                            unit={massUnitForDisplay}
                                            disabled={gsxFuelSyncEnabled === 1}
                                        />
                                    </div>
                                    {/*  </TooltipWrapper> */}
                                </td>
                                <td className="px-4 w-20 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(leftOuterGal * FUEL_GALLONS_TO_KG)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                            <tr>
                                <td className="px-4 font-light whitespace-nowrap text-md">
                                    Right Outer
                                </td>
                                <td className="mx-8">
                                    {/* <TooltipWrapper text={`${t('Ground.Payload.TT.MaxPassengers')} ${maxPax}`}> */}
                                    <div className={`px-4 font-light whitespace-nowrap text-md ${/* (gsxPayloadSyncEnabled && boardingStarted) */ false ? 'pointer-events-none' : ''}`}>
                                        <ValueInput
                                            min={0}
                                            max={Math.ceil(Units.kilogramToUser(OUTER_TANK_MAX_KG))}
                                            value={Units.kilogramToUser(rightOuterGal * FUEL_GALLONS_TO_KG)}
                                            onBlur={(x) => {
                                                if (!Number.isNaN(parseInt(x) || parseInt(x) === 0)) {
                                                    setRightOuter(Units.userToKilogram(parseInt(x)) / FUEL_GALLONS_TO_KG);
                                                    // TODO: Remove placeholder refueling setting
                                                    setRefuelStartedByUser(true);
                                                    setRefuelRate('2');
                                                }
                                            }}
                                            unit={massUnitForDisplay}
                                            disabled={gsxFuelSyncEnabled === 1}
                                        />
                                    </div>
                                    {/*  </TooltipWrapper> */}
                                </td>
                                <td className="px-4 w-20 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(rightOuterGal * FUEL_GALLONS_TO_KG)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                            <tr>
                                <td className="px-4 font-light whitespace-nowrap text-md">
                                    Trim
                                </td>
                                <td className="mx-8">
                                    {/* <TooltipWrapper text={`${t('Ground.Payload.TT.MaxPassengers')} ${maxPax}`}> */}
                                    <div className={`px-4 font-light whitespace-nowrap text-md ${/* (gsxPayloadSyncEnabled && boardingStarted) */ false ? 'pointer-events-none' : ''}`}>
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
                                            disabled={gsxFuelSyncEnabled === 1}
                                        />
                                    </div>
                                    {/*  </TooltipWrapper> */}
                                </td>
                                <td className="px-4 w-20 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(trimGal * FUEL_GALLONS_TO_KG)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                            <tr>
                                <td className="px-4 font-light whitespace-nowrap text-md">
                                    Total Fuel
                                </td>
                                <td className="mx-8">
                                    {/* <TooltipWrapper text={`${t('Ground.Payload.TT.MaxPassengers')} ${maxPax}`}> */}
                                    <div className={`px-4 font-light whitespace-nowrap text-md ${/* (gsxPayloadSyncEnabled && boardingStarted) */ false ? 'pointer-events-none' : ''}`}>
                                        <ValueInput
                                            min={0}
                                            max={Math.ceil(Units.kilogramToUser(TOTAL_FUEL_KG))}
                                            value={Units.kilogramToUser(totalFuelWeightKg)}
                                            onBlur={(x) => {
                                                if (!Number.isNaN(parseInt(x) || parseInt(x) === 0)) {
                                                    setDesiredFuelWeight(Units.userToKilogram(parseInt(x)));
                                                    // TODO: Remove placeholder refueling setting
                                                    setRefuelStartedByUser(true);
                                                    setRefuelRate('2');
                                                }
                                            }}
                                            unit={massUnitForDisplay}
                                            disabled={gsxFuelSyncEnabled === 1}
                                        />
                                    </div>
                                    {/*  </TooltipWrapper> */}
                                </td>
                                <td className="px-4 w-20 font-mono font-light whitespace-nowrap text-md">
                                    <ValueUnitDisplay value={Units.kilogramToUser(totalFuelWeightKg)} padTo={6} unit={massUnitForDisplay} />
                                </td>
                            </tr>
                        </tbody>
                    </table>
                </Card>
                {/*
                    {showSimbriefButton
                        && (
                            <TooltipWrapper text={t('Ground.Payload.TT.FillPayloadFromSimbrief')}>
                                <div
                                    className={`flex justify-center items-center px-2 h-auto text-theme-body
                                                hover:text-theme-highlight bg-theme-highlight hover:bg-theme-body
                                                rounded-md rounded-l-none border-2 border-theme-highlight transition duration-100`}
                                    onClick={setSimBriefValues}
                                >
                                    <CloudArrowDown size={26} />
                                </div>
                            </TooltipWrapper>
                        )}
                        */}
            </div>
        </div>
    );
};
