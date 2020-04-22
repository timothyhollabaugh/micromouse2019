import serial
from more_itertools import *
import matplotlib.pyplot as plt
from matplotlib import collections
import control


def step_motor(s, before_time, step_time, after_time):
    s.write(b'time report on\n')
    s.write(b'motor left report on\n')

    start_time = None

    step = 0

    motor_positions = []

    while True:
        line = s.readline()
        #print(line)
        if b',' in line and b':' in line:
            time = None
            position = None

            words = line.split(b',')
            for word in words:
                parts = word.split(b':')
                if parts[0] == b'T':
                    time = int(parts[1])
                elif parts[0] == b'LM':
                    position = int(parts[1])

            if start_time is None:
                start_time = time
            else:
                if time is not None and position is not None:
                    motor_positions.append({
                        'time': time,
                        'position': position,
                        'step': step,
                    })

                if step == 0 and time - start_time > before_time:
                    s.write(b'motor left set 10000\n')
                    start_time = time
                    step = 1
                elif step == 1 and time - start_time > step_time:
                    s.write(b'motor left set 0\n')
                    start_time = time
                    step = 2
                elif step == 2 and time - start_time > after_time:
                    s.write(b'motor left report off\n')
                    s.write(b'time report off\n')
                    break

    return motor_positions


def calc_velocity(positions):
    current_position, next_position = positions
    delta_time = next_position['time'] - current_position['time']
    delta_position = next_position['position'] - current_position['position']
    return {
        'time': current_position['time'],
        'step': current_position['step'],
        'velocity': delta_position / delta_time,
    }


def to_velocity(motor_positions):
    return list(map(calc_velocity, windowed(motor_positions, 2)))


def calc_final_velocity(velocities, time):
    """
    Calculate the final veclocity as an average over `time` of the last velocities in the run step
    :param velocities: A list of dictionaries with `time`, `step`, and `velocity` keys
    :param time: How much time of samples to average over
    :return: The average velocity as the end of the run step
    """
    last_time = last(velocities)['time']
    final_velocities = list(filter(lambda d: last_time - d['time'] <= time, velocities));
    final_average = sum(map(lambda d: d['velocity'], final_velocities)) / len(final_velocities)
    return final_average


def time_at_velocity(velocities, velocity):
    """
    Calculate the time when the velocity in `velocities` first crosses `velocity` with a rising edge
    :param velocities: A list of dictionaries with `time` and `velocity` keys
    :param velocity: The velocity to calculate the time of when velocities crosses it
    :return: The time of crossing
    """

    return next(map(lambda d: d[0]['time'], filter(lambda d: d[0]['velocity'] <= velocity < d[1]['velocity'],
                                                   windowed(velocities, 2))))


def extract(data, key):
    return list(map(lambda d: d[key], data))


def plot_steps(ax, times, steps, ymin, ymax):
    steps0 = list(map(lambda s: s == 0, steps))
    steps1 = list(map(lambda s: s == 1, steps))
    steps2 = list(map(lambda s: s == 2, steps))

    steps0_collection = collections.BrokenBarHCollection.span_where(times, ymin=ymin, ymax=ymax,
                                                                    where=steps0,
                                                                    facecolor='yellow',
                                                                    alpha=0.2)

    steps1_collection = collections.BrokenBarHCollection.span_where(times, ymin=ymin, ymax=ymax,
                                                                    where=steps1,
                                                                    facecolor='green',
                                                                    alpha=0.2)

    steps2_collection = collections.BrokenBarHCollection.span_where(times, ymin=ymin, ymax=ymax,
                                                                    where=steps2,
                                                                    facecolor='red',
                                                                    alpha=0.2)

    ax.add_collection(steps0_collection)
    ax.add_collection(steps1_collection)
    ax.add_collection(steps2_collection)


s = serial.Serial(port='/dev/ttyUSB0', baudrate=230400, timeout=1)

p = step_motor(s, 100, 500, 0)

v = to_velocity(p)

time_start = time_at_velocity(v, 0)

v_run = list(map(lambda d: {'time': d['time'] - time_start, 'velocity': d['velocity']},
                 filter(lambda d: d['time'] >= time_start, v)))

times = extract(v_run, 'time')
velocities = extract(v_run, 'velocity')

final_v = calc_final_velocity(v_run, 200)
ta_v = 0.632 * final_v

ta = time_at_velocity(v_run, ta_v)

s = control.tf('s')

tf = final_v / (ta * s + 1)

print(final_v, ta)

step_times, step_response = control.step_response(tf, times)

fig, ax1 = plt.subplots(1, 1)

ax1.plot(times, velocities)
#ax1.plot(times, list(ncycles([final_v], len(times))))
#ax1.plot(times, list(ncycles([ta_v], len(times))))
#ax1.axvline(ta, 0, 10, color='red')
ax1.plot(times, step_response, color='yellow')

plt.show()
